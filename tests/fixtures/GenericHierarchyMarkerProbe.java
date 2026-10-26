abstract class MarkerRoot<T> {
    int value = 3;
    abstract int tag();
    int read() { return value; }
    void set(int next) { value = next; }
    public int hashCode() { return value; }
}

// Exercise a source subtype whose name collides with the marker's first choice.
class __JavaTypeMarker extends MarkerRoot<String> {
    int tag() { return 1; }
}

class OtherMarker extends MarkerRoot<Integer> {
    int tag() { return 2; }
}

class GenericHierarchyMarkerProbe {
    static int run() {
        MarkerRoot<String> first = new __JavaTypeMarker();
        MarkerRoot<String> alias = first;
        alias.set(9);
        if (first.read() != 9 || first.tag() != 1) return 1;
        return first.hashCode() == 9 && first == alias ? 0 : 2;
    }

    public static void main(String[] args) { System.out.println(run()); }
}
