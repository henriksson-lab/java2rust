import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;

class NullableCollectionDisplay {
    static ArrayList<Integer> field;
    static ArrayList<Integer> values() {
        ArrayList<Integer> result = new ArrayList<>();
        result.add(3); result.add(1);
        return result;
    }
    static String list(ArrayList<Integer> values) { return "list=" + values; }
    static String set(LinkedHashSet<Integer> values) { return "set=" + values; }
    static String map(LinkedHashMap<String, Integer> values) { return "map=" + values; }
    static String formatted(ArrayList<Integer> values) { return String.format("<%s>", values); }
    static String repeated(ArrayList<Integer> values) { return "A=" + values + ";B=" + values; }
    static String probe(boolean present) {
        ArrayList<Integer> list = null;
        LinkedHashSet<Integer> set = null;
        LinkedHashMap<String, Integer> map = null;
        if (present) {
            list = values();
            set = new LinkedHashSet<>(); set.add(3); set.add(1);
            map = new LinkedHashMap<>(); map.put("k", 5);
        }
        return list(list) + ";" + set(set) + ";" + map(map) + ";" + formatted(list) + ";" + repeated(list);
    }
    static ArrayList<Integer> nullableValues(boolean present) {
        if (present) return values();
        return null;
    }
    static String fromReturn(boolean present) { return "R=" + nullableValues(present); }
    static String fromField(boolean present) {
        field = nullableValues(present);
        return "F=" + field;
    }
    static String fromMap(boolean present) {
        LinkedHashMap<String, ArrayList<Integer>> map = new LinkedHashMap<>();
        if (present) map.put("k", values());
        return "M=" + map.get("k");
    }
    static int directNull() {
        ArrayList<Integer> values = null;
        try { values.toString(); return -1; }
        catch (NullPointerException error) { return 8; }
    }
}
