import java.util.List;
import java.util.ArrayList;
class PlaceholderRoot<T> {
    List<T> values;
    PlaceholderRoot(List<T> initial) { this.values = new ArrayList<T>(); this.values.addAll(initial); }
    int size() { return values.size(); }
    void append(T item) { values.add(item); }
}
class PlaceholderChild<T> extends PlaceholderRoot<T> {
    PlaceholderChild(List<T> initial) { super(initial); }
}
class PlaceholderOwner<T> {
    PlaceholderRoot<T> value;
    PlaceholderOwner(PlaceholderRoot<T> value) { this.value = value; }
    int size() { return value.size(); }
}
class GenericPlaceholderDefaultProbe {
    static int run() {
        List<String> values = new ArrayList<String>();
        values.add("one");
        PlaceholderRoot<String> root = new PlaceholderRoot<String>(values);
        PlaceholderRoot<String> child = new PlaceholderChild<String>(values);
        PlaceholderOwner<String> owner = new PlaceholderOwner<String>(root);
        values.add("two");
        if (owner.size() != 1 || root.size() != 1 || child.size() != 1) return 1;
        root.append("three");
        if (owner.size() != 2 || root.size() != 2 || child.size() != 1) return 2;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
