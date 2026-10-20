import java.util.*;
class SemanticSetCopy {
    static <T> Set<T> copy(Collection<T> values) {
        Set<T> result = new HashSet<T>(values);
        return result;
    }
    static <T> Set<T> copySet(Set<T> values) {
        return new HashSet<T>(values);
    }
    static <T> Set<T> widen(T value) {
        Set<T> result = new HashSet<T>(Arrays.asList(value));
        result.add(null);
        return result;
    }
    static <T> Map<T, Integer> copyMap(Map<T, Integer> values) {
        return new HashMap<T, Integer>(values);
    }
    static int run() {
        List<Double> values = new ArrayList<Double>();
        values.add(Double.NaN);
        values.add(-0.0);
        values.add(0.0);
        values.add(Double.NaN);
        Set<Double> original = copy(values);
        Set<Double> copied = copySet(original);
        if (original.size() != 3 || copied.size() != 3) return 1;
        if (!copied.contains(Double.NaN) || !copied.contains(-0.0) || !copied.contains(0.0)) return 2;
        copied.remove(-0.0);
        if (copied.size() != 2 || !copied.contains(0.0) || copied.contains(-0.0)) return 3;
        if (original.size() != 3 || values.size() != 4) return 4;
        if (widen(Double.NaN).size() != 2) return 5;
        Map<Double, Integer> map = new HashMap<Double, Integer>();
        map.put(Double.NaN, 1);
        map.put(-0.0, 2);
        map.put(0.0, 3);
        map.put(Double.NaN, 4);
        Map<Double, Integer> mapCopy = copyMap(map);
        mapCopy.remove(-0.0);
        if (map.size() != 3 || mapCopy.size() != 2 || !mapCopy.containsKey(0.0)) return 6;
        return 0;
    }
}
