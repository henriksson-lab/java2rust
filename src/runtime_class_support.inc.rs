fn java_runtime_class_canonical_name(name: &str) -> String {
    if let Some(component)=name.strip_suffix("[]") {
        let component=java_runtime_class_canonical_name(component);
        return if component.starts_with('[') { format!("[{component}") }
        else if let Some(code)=java_runtime_class_primitive_code(&component) { format!("[{code}") }
        else { format!("[L{component};") };
    }
    if let Some(component)=name.strip_prefix("[L").and_then(|v|v.strip_suffix(';')) {
        return format!("[L{};",java_runtime_class_canonical_name(component));
    }
    if let Some(component)=name.strip_prefix("[[") { return format!("[{}",java_runtime_class_canonical_name(&format!("[{component}"))); }
    match name {
/* CLASS_ALIASES */
        _ => name,
    }.to_string()
}
fn java_runtime_class_primitive_code(name: &str) -> Option<char> {
    Some(match name { "boolean"=>'Z',"byte"=>'B',"short"=>'S',"char"=>'C',"int"=>'I',"long"=>'J',"float"=>'F',"double"=>'D',"void"=>'V',_=>return None })
}
fn java_runtime_class_enum_kind(name: &str) -> Option<bool> {
    if name.starts_with('[') || java_runtime_class_primitive_code(name).is_some() { return Some(false); }
    match name {
/* CLASS_ENUM_KIND */
        _ => None,
    }
}
fn java_runtime_class_component(name: &str) -> Option<String> {
    let component=name.strip_prefix('[')?;
    Some(if let Some(value)=component.strip_prefix('L').and_then(|v|v.strip_suffix(';')) {value.to_string()}
    else if component.starts_with('[') {component.to_string()}
    else {match component {"Z"=>"boolean","B"=>"byte","S"=>"short","C"=>"char","I"=>"int","J"=>"long","F"=>"float","D"=>"double",_=>return None}.to_string()})
}
fn java_reference_array_store_check(
    class: Option<&str>, index: i32, length: usize, actual: Option<&str>,
) -> Result<(), JavaException> {
    if index < 0 || index as usize >= length {
        return Err(JavaException::new("ArrayIndexOutOfBoundsException", index.to_string()));
    }
    // Null is assignable to every reference component, even if a class is
    // unavailable at an otherwise valid typed boundary.
    let Some(actual) = actual else { return Ok(()); };
    let component = class.and_then(java_runtime_class_component).ok_or_else(||
        JavaException::new("UnsupportedOperationException", "Missing reference array component metadata"))?;
    if java_runtime_class_is_assignable(&component, actual)? { Ok(()) }
    else { Err(JavaException::new("ArrayStoreException", actual)) }
}
fn java_runtime_class_parents(name: &str) -> &'static [&'static str] {
    match name {
/* CLASS_PARENTS */
        _ => &[],
    }
}
fn java_runtime_class_ancestry_complete(name: &str) -> bool {
    match name {
/* CLASS_COMPLETE */
        _ => false,
    }
}
fn java_runtime_class_is_assignable(target: &str,actual: &str) -> Result<bool,JavaException> {
    if target==actual { return Ok(true); }
    if java_runtime_class_primitive_code(target).is_some() || java_runtime_class_primitive_code(actual).is_some() {return Ok(false);}
    if target=="java.lang.Object" {return Ok(true);}
    if java_runtime_class_is_final(target) {return Ok(false);}
    if let Some(actual_component)=java_runtime_class_component(actual) {
        if matches!(target,"java.lang.Cloneable"|"java.io.Serializable") {return Ok(true);}
        return match java_runtime_class_component(target) {
            Some(component)=>java_runtime_class_is_assignable(&component,&actual_component),
            None=>Ok(false),
        };
    }
    if target.starts_with('[') {return Ok(false);}
    let mut pending=vec![actual];
    let mut visited=std::collections::HashSet::new();
    let mut incomplete=false;
    while let Some(current)=pending.pop() {
        if !visited.insert(current) {continue;}
        if current==target {return Ok(true);}
        // Distinct source enum declarations cannot inherit from each other.
        // Stop at that proved boundary, including a constant-specific class's
        // declaring enum, without requiring a complete platform Enum ancestry.
        if java_runtime_class_enum_kind(target)==Some(true)
            && java_runtime_class_enum_kind(current)==Some(true) {continue;}
        incomplete |= !java_runtime_class_ancestry_complete(current);
        pending.extend(java_runtime_class_parents(current).iter().copied());
    }
    if incomplete {Err(JavaException::new("UnsupportedOperationException",format!("Java class ancestry metadata is not registered for {actual} -> {target}")))}
    else {Ok(false)}
}
fn java_runtime_class_is_final(name: &str) -> bool {
    match name {
/* CLASS_FINAL */
        _ => false,
    }
}
pub trait JavaClassArgument {
    fn java_class_argument_name(self) -> Result<String,JavaException>;
}
impl<T> JavaClassArgument for JavaClass<T> {
    fn java_class_argument_name(self)->Result<String,JavaException> {Ok(self.name)}
}
impl<T> JavaClassArgument for &JavaClass<T> {
    fn java_class_argument_name(self)->Result<String,JavaException> {Ok(self.name.clone())}
}
impl<T> JavaClassArgument for &mut JavaClass<T> {
    fn java_class_argument_name(self)->Result<String,JavaException> {Ok(self.name.clone())}
}
impl<C:JavaClassArgument> JavaClassArgument for Option<C> {
    fn java_class_argument_name(self)->Result<String,JavaException> {
        self.ok_or_else(||JavaException::new("NullPointerException","Java null class argument".to_string()))?.java_class_argument_name()
    }
}
