import java.util.Arrays;
import java.util.Iterator;

public class ErasedCapabilityTypeContracts {
    static class Rows implements Iterable<String> {
        public Iterator<String> iterator() { return Arrays.asList("row").iterator(); }
    }
    static boolean iterable(Object value) { return value instanceof Iterable; }
    static boolean wildcard(Object value) { return value instanceof Iterable<?>; }
    static boolean array(Object value) { return value instanceof Object[]; }
    static int run() {
        String[] values = new String[]{"row", null};
        Object erased = values;
        if (!array(erased) || iterable(erased) || wildcard(erased)) return 1;
        Object primitive = new int[]{1};
        if (array(primitive) || iterable(primitive)) return 2;
        try { Object negative = new int[-1]; return 6; }
        catch (NegativeArraySizeException expected) { }
        if (array(null) || iterable(null) || wildcard(null)) return 3;
        Object text = "text";
        if (array(text) || iterable(text)) return 4;
        Object rows = new Rows();
        if (!iterable(rows) || !wildcard(rows) || array(rows)) return 5;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
