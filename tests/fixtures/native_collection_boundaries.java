import java.util.List;
import java.util.ArrayList;
import java.util.Set;
import java.util.HashSet;
import java.util.Map;
import java.util.HashMap;
class SetBoundaryReader {
    int count;
    SetBoundaryReader(Set<? extends String> values) { count = values.size(); }
    static int read(Set<String> values) { return values.size(); }
}
class NativeCollectionBoundaries {
    static Set<String> pass(Set<String> values) { return values; }
    static <T> T identity(T value) { return value; }
    static int run() {
        List<Set<String>> groups = new ArrayList<>();
        Set<String> original = new HashSet<>();
        groups.add(original);
        int result = 0;
        for (Set<String> group : groups) {
            pass(group).add("kept");
            SetBoundaryReader reader = new SetBoundaryReader(group);
            result = reader.count + SetBoundaryReader.read(group);
        }
        Map<String, Set<String>> byName = new HashMap<>();
        byName.put("key", original);
        Set<String> retrieved = byName.get("key");
        pass(retrieved).add("from-map");
        Set<String> forwarded = identity(original);
        forwarded.add("from-generic");
        Set<String> copy = new HashSet<>(original);
        copy.add("copy-only");
        Set<String> equal = new HashSet<>();
        equal.add("from-map"); equal.add("kept"); equal.add("from-generic");
        boolean matched = groups.contains(equal);
        int position = groups.indexOf(equal);
        boolean removed = groups.remove(equal);
        return original.contains("kept") && original.contains("from-map") && original.contains("from-generic")
            && !original.contains("copy-only") && matched && position == 0 && removed && groups.isEmpty()
            ? result : -1;
    }
}
