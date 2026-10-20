import java.util.*;

class CollectionDisplayElement {
    CollectionDisplayOwner owner;
    String text;
    CollectionDisplayElement(CollectionDisplayOwner owner, String text) {
        this.owner = owner; this.text = text;
    }
    public String toString() {
        owner.calls++;
        if (owner.fail) throw new IllegalStateException("element display");
        if (owner.clear) owner.values.clear();
        return text;
    }
}
class CollectionDisplayOwner {
    List<CollectionDisplayElement> values = new ArrayList<>();
    int calls;
    boolean fail;
    boolean clear;
    List<CollectionDisplayElement> alias() { return values; }
    String render() { return values.toString(); }
    String concatenate() { return "values=" + values; }
    String suffix() { calls += 100; return "!"; }
    String concatenateThenCall() { return "values=" + values + suffix(); }
}
class GenericCollectionDisplay<T> {
    List<T> values;
    GenericCollectionDisplay(List<T> initial) { values = initial; }
    String render() { return "generic=" + values; }
}
class GenericDisplayElement<T> {
    T value;
    boolean nullText;
    boolean fail;
    GenericDisplayElement(T value) { this.value = value; }
    public String toString() {
        if (fail) throw new IllegalArgumentException("typed display");
        if (nullText) return null;
        return "typed-" + value;
    }
}
class ParameterDisplayBase<T> {
    List<T> retained;
    String render(List<T> input) {
        retained = input;
        return "parameter=" + input;
    }
    String render(int ignored) { return "unused"; }
}
class ParameterDisplayChild<U> extends ParameterDisplayBase<U> { }
interface CollectionRenderer<T> { String render(); }
class GenericRenderer<T> implements CollectionRenderer<T> {
    List<T> values;
    GenericRenderer(List<T> input) { values = input; }
    public String render() { return "renderer=" + values; }
}
class ProtocolDisplayBase<T> {
    List<T> values;
    void set(List<T> input) { values = input; }
    public String toString() { return "protocol=" + values; }
}
class ProtocolDisplayChild<U> extends ProtocolDisplayBase<U> { }
class SyncCollectionDisplay {
    static List<Integer> values = new ArrayList<>();
    static String render() { return "sync=" + values; }
    static int run() {
        values.clear(); values.add(3); values.add(null);
        List<Integer> alias = values;
        if (!render().equals("sync=[3, null]") || !alias.toString().equals("[3, null]")) return 1;
        return 0;
    }
}
class FieldDisplayElement {
    FieldDisplayOwner owner;
    FieldDisplayElement(FieldDisplayOwner owner) { this.owner = owner; }
    public String toString() {
        owner.calls++;
        if (owner.clear) owner.items.clear();
        return "field";
    }
}
class FieldDisplayOwner {
    List<FieldDisplayElement> items = new ArrayList<>();
    int calls;
    boolean clear;
    void add() { items.add(new FieldDisplayElement(this)); }
    boolean empty() { return items.isEmpty(); }
    String render() { return "owner=" + items; }
    static int run() {
        FieldDisplayOwner owner = new FieldDisplayOwner();
        owner.add(); owner.add();
        if (!owner.render().equals("owner=[field, field]") || owner.calls != 2) return 1;
        owner.clear = true;
        try { owner.render(); return 2; }
        catch (ConcurrentModificationException expected) { }
        if (!owner.empty() || owner.calls != 3) return 3;
        return 0;
    }
}
class RetainedCollectionFormatting {
    static <T> String renderThroughInterface(List<T> values) {
        CollectionRenderer<T> renderer = new GenericRenderer<>(values);
        return renderer.render();
    }
    static String nullable(List<CollectionDisplayElement> values) {
        return "nullable=" + values;
    }
    static int run() {
        CollectionDisplayOwner owner = new CollectionDisplayOwner();
        List<CollectionDisplayElement> alias = owner.alias();
        CollectionDisplayElement first = new CollectionDisplayElement(owner, "one");
        alias.add(first);
        alias.add(new CollectionDisplayElement(owner, "two"));
        if (!owner.render().equals("[one, two]") || owner.calls != 2) return 1;
        first.text = null;
        if (!owner.render().equals("[null, two]") || owner.calls != 4) return 2;
        owner.fail = true;
        try { owner.render(); return 3; }
        catch (IllegalStateException expected) { }
        owner.fail = false; owner.clear = true; owner.calls = 0;
        try { owner.render(); return 4; }
        catch (ConcurrentModificationException expected) { }
        if (!alias.isEmpty() || owner.calls != 1) return 5;
        owner.clear = false; owner.calls = 0; first.text = "one";
        alias.add(first);
        alias.add(new CollectionDisplayElement(owner, "two"));
        if (!owner.concatenate().equals("values=[one, two]") || owner.calls != 2) return 6;
        GenericCollectionDisplay<CollectionDisplayElement> generic = new GenericCollectionDisplay<>(alias);
        if (!generic.render().equals("generic=[one, two]") || owner.calls != 4) return 7;
        owner.calls = 0;
        List<List<CollectionDisplayElement>> nested = new ArrayList<>();
        nested.add(alias);
        if (!("nested=" + nested).equals("nested=[[one, two]]") || owner.calls != 2) return 15;
        owner.calls = 0;
        List<CollectionDisplayElement> view = Collections.unmodifiableList(alias);
        if (!("view=" + view).equals("view=[one, two]") || owner.calls != 2) return 16;
        owner.calls = 0;
        Map<String, CollectionDisplayElement> map = new LinkedHashMap<>();
        Map<String, CollectionDisplayElement> mapAlias = map;
        map.put("first", first); map.put("second", alias.get(1));
        if (!("map=" + map).equals("map={first=one, second=two}") || owner.calls != 2) return 17;
        if (!mapAlias.toString().equals("{first=one, second=two}") || owner.calls != 4) return 18;
        if (!("map-values=" + map.values()).equals("map-values=[one, two]") || owner.calls != 6) return 19;
        if (SyncCollectionDisplay.run() != 0) return 20;
        if (FieldDisplayOwner.run() != 0) return 21;
        owner.calls = 0;
        if (!String.valueOf(alias).equals("[one, two]") || owner.calls != 2) return 22;
        owner.calls = 0;
        if (!Objects.toString(alias, owner.suffix()).equals("[one, two]") || owner.calls != 102) return 23;
        if (!Objects.toString(null, "fallback").equals("fallback")) return 24;
        List<GenericDisplayElement<String>> typedValues = new ArrayList<>();
        GenericDisplayElement<String> typed = new GenericDisplayElement<>("value");
        typedValues.add(typed);
        GenericCollectionDisplay<GenericDisplayElement<String>> typedDisplay = new GenericCollectionDisplay<>(typedValues);
        if (!typedDisplay.render().equals("generic=[typed-value]")) return 25;
        typed.nullText = true;
        if (!typedDisplay.render().equals("generic=[null]")) return 26;
        typed.fail = true;
        try { typedDisplay.render(); return 27; }
        catch (IllegalArgumentException expected) { }
        ParameterDisplayChild<String> parameterDisplay = new ParameterDisplayChild<>();
        List<String> parameterValues = new ArrayList<>();
        parameterValues.add("before");
        if (!parameterDisplay.render(parameterValues).equals("parameter=[before]")) return 28;
        parameterValues.add("after");
        if (!parameterDisplay.retained.toString().equals("[before, after]")) return 29;
        if (!parameterDisplay.render(0).equals("unused")) return 30;
        if (!renderThroughInterface(parameterValues).equals("renderer=[before, after]")) return 31;
        ProtocolDisplayChild<String> protocol = new ProtocolDisplayChild<>();
        protocol.set(parameterValues);
        if (!protocol.toString().equals("protocol=[before, after]")) return 32;
        CollectionRenderer<GenericDisplayElement<String>> throwingRenderer = new GenericRenderer<>(typedValues);
        try { throwingRenderer.render(); return 33; }
        catch (IllegalArgumentException expected) { }
        typed.fail = false; typed.nullText = false;
        typedValues.add(null);
        if (!typedDisplay.render().equals("generic=[typed-value, null]")) return 34;
        if (!throwingRenderer.render().equals("renderer=[typed-value, null]")) return 35;
        if (!nullable(null).equals("nullable=null")) return 8;
        owner.calls = 0;
        if (!owner.concatenateThenCall().equals("values=[one, two]!") || owner.calls != 102) return 9;
        owner.calls = 0; owner.fail = true;
        try { owner.concatenateThenCall(); return 10; }
        catch (IllegalStateException expected) { }
        if (owner.calls != 1) return 11;
        try { generic.render(); return 12; }
        catch (IllegalStateException expected) { }
        owner.fail = false; owner.clear = true; owner.calls = 0;
        try { owner.concatenate(); return 13; }
        catch (ConcurrentModificationException expected) { }
        if (!alias.isEmpty() || owner.calls != 1) return 14;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
