import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.NavigableMap;
import java.util.SortedMap;
import java.util.TreeMap;

public class InterfaceMapEntryParameterMutation {
    static String mutate(Map<String, Integer> values, int next) {
        Map.Entry<String, Integer> entry = values.entrySet().iterator().next();
        Integer old = entry.setValue(next);
        return old + ":" + values.size() + ":" + entry.getValue();
    }

    static String mutateSorted(SortedMap<String, Integer> values, int next) {
        Map.Entry<String, Integer> entry = values.entrySet().iterator().next();
        Integer old = entry.setValue(next);
        return old + ":" + values.size() + ":" + entry.getValue();
    }

    static String mutateNavigable(NavigableMap<String, Integer> values, int next) {
        Map.Entry<String, Integer> entry = values.entrySet().iterator().next();
        Integer old = entry.setValue(next);
        return old + ":" + values.size() + ":" + entry.getValue();
    }

    public static String run() {
        HashMap<String, Integer> hash = new HashMap<>();
        hash.put("h", 1);
        String hashResult = mutate(hash, 7);
        LinkedHashMap<String, Integer> linked = new LinkedHashMap<>();
        linked.put("l", 2);
        String linkedResult = mutate(linked, 8);
        TreeMap<String, Integer> tree = new TreeMap<>();
        tree.put("a", 3);
        String treeResult = mutate(tree, 9);
        HashMap<String, Integer> nullableHash = new HashMap<>();
        nullableHash.put(null, null);
        String nullableHashResult = mutate(nullableHash, 10);
        LinkedHashMap<String, Integer> nullableLinked = new LinkedHashMap<>();
        nullableLinked.put(null, null);
        String nullableLinkedResult = mutate(nullableLinked, 11);
        TreeMap<String, Integer> nullableTree = new TreeMap<>();
        nullableTree.put("a", null);
        String nullableTreeResult = mutate(nullableTree, 12);
        TreeMap<String, Integer> sorted = new TreeMap<>();
        sorted.put("a", 13);
        String sortedResult = mutateSorted(sorted, 14);
        TreeMap<String, Integer> navigable = new TreeMap<>();
        navigable.put("a", 15);
        String navigableResult = mutateNavigable(navigable, 16);
        return hashResult + "," + linkedResult + "," + treeResult
            + "," + nullableHashResult + "," + nullableLinkedResult
            + "," + nullableTreeResult + "," + sortedResult
            + "," + navigableResult;
    }

    public static void main(String[] args) { System.out.println(run()); }
}
