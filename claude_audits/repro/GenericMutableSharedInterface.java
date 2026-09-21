interface GenericCell<T> {
    void set(T value);
    T get();
}

class GenericStringCell implements GenericCell<String> {
    String value;

    GenericStringCell(String value) { this.value = value; }
    public void set(String value) { this.value = value; }
    public String get() { return this.value; }
}

public class GenericMutableSharedInterface {
    static <T extends GenericCell<String>> String replace(T cell) {
        cell.set("right");
        return cell.get();
    }

    static String run() {
        GenericStringCell first = new GenericStringCell("left");
        GenericStringCell second = first;
        replace(first);
        return second.get();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
