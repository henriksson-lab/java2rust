import java.util.Arrays;
import java.util.Collection;
import java.util.List;

class InlineArrayHolder<T> {
    List<T> items;
    InlineArrayHolder(List<T> items) { this.items = items; }
}

class InlineArrayFactory<T> {
    List<T> keep(List<T> items) { return items; }
    int count(Collection<? extends T> items) { return items.size(); }
    List<T> fromArray(T[] values) { return keep(Arrays.asList(values)); }
    InlineArrayHolder<T> make(T[] values) {
        return new InlineArrayHolder<T>(Arrays.asList(values));
    }
    int read(T[] values) { return count(Arrays.asList(values)); }
}

class InlineArrayFactoryBoundaries {
    static int run() {
        String[] values = new String[] { "before" };
        InlineArrayFactory<String> factory = new InlineArrayFactory<>();
        List<String> first = factory.fromArray(values);
        first.set(0, "first");
        InlineArrayHolder<String> holder = factory.make(values);
        holder.items.set(0, "second");
        if (!values[0].equals("second")) return -1;
        return factory.read(values);
    }
}
