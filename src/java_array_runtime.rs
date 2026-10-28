// Runtime class evidence belongs to the original allocation, including empty
// arrays. Weak owners prevent address reuse from changing another array's class.
// Runtime-created descriptors (for example reflection arrays) have the same
// VM lifetime as literal class descriptors. Intern once per distinct class,
// rather than leaking a descriptor for every fresh array.
fn java_intern_array_runtime_class(name: &str) -> &'static str {
    static NAMES: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<&'static str>>> =
        std::sync::OnceLock::new();
    let mut names = NAMES.get_or_init(Default::default).lock().unwrap();
    if let Some(existing) = names.get(name) {
        return existing;
    }
    let name = Box::leak(name.to_owned().into_boxed_str());
    names.insert(name);
    name
}

struct JavaLocalArrayClass {
    owner: std::rc::Weak<dyn std::any::Any>,
    name: &'static str,
}

struct JavaSyncArrayClass {
    owner: std::sync::Weak<dyn std::any::Any + Send + Sync>,
    name: &'static str,
}

std::thread_local! {
    static JAVA_LOCAL_ARRAY_CLASSES: std::cell::RefCell<std::collections::HashMap<usize, JavaLocalArrayClass>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

fn java_sync_array_classes()
-> &'static std::sync::Mutex<std::collections::HashMap<usize, JavaSyncArrayClass>> {
    static CLASSES: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<usize, JavaSyncArrayClass>>,
    > = std::sync::OnceLock::new();
    CLASSES.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// Only fresh producers may attach class evidence. Aliases retain that evidence
/// and must never register their declared (possibly covariant) destination type.
pub fn java_register_local_array_class<T: 'static>(
    owner: std::rc::Rc<T>,
    name: &str,
) -> std::rc::Rc<T> {
    let name = java_intern_array_runtime_class(name);
    let identity = std::rc::Rc::as_ptr(&owner) as usize;
    let erased: std::rc::Rc<dyn std::any::Any> = owner.clone();
    JAVA_LOCAL_ARRAY_CLASSES.with(|classes| {
        let mut classes = classes.borrow_mut();
        // Amortize cleanup while bounding dead entries between collections.
        if classes.len() % 64 == 0 {
            classes.retain(|_, entry| entry.owner.strong_count() != 0);
        }
        if let Some(previous) = classes.get(&identity) {
            assert_eq!(previous.name, name, "relabeling a live Java array");
        } else {
            classes.insert(
                identity,
                JavaLocalArrayClass {
                    owner: std::rc::Rc::downgrade(&erased),
                    name,
                },
            );
        }
    });
    owner
}

pub fn java_register_sync_array_class<T: Send + Sync + 'static>(
    owner: std::sync::Arc<T>,
    name: &str,
) -> std::sync::Arc<T> {
    let name = java_intern_array_runtime_class(name);
    let identity = std::sync::Arc::as_ptr(&owner) as usize;
    let erased: std::sync::Arc<dyn std::any::Any + Send + Sync> = owner.clone();
    let mut classes = java_sync_array_classes().lock().unwrap();
    if classes.len() % 64 == 0 {
        classes.retain(|_, entry| entry.owner.strong_count() != 0);
    }
    if let Some(previous) = classes.get(&identity) {
        assert_eq!(previous.name, name, "relabeling a live Java array");
    } else {
        classes.insert(
            identity,
            JavaSyncArrayClass {
                owner: std::sync::Arc::downgrade(&erased),
                name,
            },
        );
    }
    owner
}

/// None is missing evidence, not proof that the value is not an array.
pub fn java_array_runtime_class(identity: usize) -> Option<&'static str> {
    let local = JAVA_LOCAL_ARRAY_CLASSES.with(|classes| {
        let mut classes = classes.borrow_mut();
        match classes.get(&identity) {
            Some(entry) if entry.owner.strong_count() != 0 => Some(entry.name),
            Some(_) => {
                classes.remove(&identity);
                None
            }
            None => None,
        }
    });
    local.or_else(|| {
        let mut classes = java_sync_array_classes().lock().unwrap();
        match classes.get(&identity) {
            Some(entry) if entry.owner.strong_count() != 0 => Some(entry.name),
            Some(_) => {
                classes.remove(&identity);
                None
            }
            None => None,
        }
    })
}

/// A copy gets a new allocation identity, but keeps its source's actual class.
/// Missing source evidence stays missing, including at generic boundaries.
pub fn java_inherit_local_array_class<T: 'static>(
    owner: std::rc::Rc<T>,
    source: Option<*const ()>,
) -> std::rc::Rc<T> {
    match source.and_then(|identity| java_array_runtime_class(identity as usize)) {
        Some(name) => java_register_local_array_class(owner, name),
        None => owner,
    }
}

pub fn java_inherit_sync_array_class<T: Send + Sync + 'static>(
    owner: std::sync::Arc<T>,
    source: Option<*const ()>,
) -> std::sync::Arc<T> {
    match source.and_then(|identity| java_array_runtime_class(identity as usize)) {
        Some(name) => java_register_sync_array_class(owner, name),
        None => owner,
    }
}

// Admission at the call site proves an exact final reference component and
// canonical element copies. Consume those copies into the existing allocation
// or into one fresh allocation; never clone or rewrap the destination payload.
pub fn java_collection_to_local_existing_array<T: 'static>(
    values: Vec<Option<T>>,
    destination: std::rc::Rc<std::cell::RefCell<Vec<Option<T>>>>,
    fresh_class: &'static str,
) -> std::rc::Rc<std::cell::RefCell<Vec<Option<T>>>> {
    let size = values.len();
    {
        let mut array = destination.borrow_mut();
        if size <= array.len() {
            for (slot, value) in array.iter_mut().zip(values) {
                *slot = value;
            }
            if size < array.len() {
                array[size] = None;
            }
            drop(array);
            return destination;
        }
    }
    java_register_local_array_class(
        std::rc::Rc::new(std::cell::RefCell::new(values)),
        fresh_class,
    )
}

pub fn java_collection_to_sync_existing_array<T: Send + Sync + 'static>(
    values: Vec<Option<T>>,
    destination: std::sync::Arc<std::sync::Mutex<Vec<Option<T>>>>,
    fresh_class: &'static str,
) -> std::sync::Arc<std::sync::Mutex<Vec<Option<T>>>> {
    let size = values.len();
    {
        let mut array = destination
            .lock()
            .expect("Java shared object mutex poisoned");
        if size <= array.len() {
            for (slot, value) in array.iter_mut().zip(values) {
                *slot = value;
            }
            if size < array.len() {
                array[size] = None;
            }
            drop(array);
            return destination;
        }
    }
    java_register_sync_array_class(
        std::sync::Arc::new(std::sync::Mutex::new(values)),
        fresh_class,
    )
}
