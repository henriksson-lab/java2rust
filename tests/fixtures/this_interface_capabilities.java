import java.util.List;
import java.util.ArrayList;
interface CapabilityView<T> {
    List<T> snapshot();
    boolean test(T value);
}
class CapabilityProbe {
    static <T> int count(CapabilityView<T> view) { return view.snapshot().size(); }
}
class CapabilitySource<T> implements CapabilityView<T> {
    List<T> values = new ArrayList<>();
    public List<T> snapshot() { return new ArrayList<>(values); }
    public boolean test(T value) { return value.equals(value); }
    void add(T value) { values.add(value); }
    CapabilityView<T> view() { return this; }
    int via() { return CapabilityProbe.count(this); }
    static int run() {
        CapabilitySource<String> source = new CapabilitySource<>();
        source.add("kept");
        return source.via() + source.view().snapshot().size();
    }
}
