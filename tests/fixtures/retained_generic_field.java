import java.util.Set;
import java.util.LinkedHashSet;
import java.util.Map;
import java.util.HashMap;

class SharedGenericSet<T> {
    Set<T> values = new LinkedHashSet<>();
    void store(Map<String, Set<T>> target) { target.put("x", values); }
    void add(T value) { values.add(value); }
    void replace(Set<T> replacement) { values = replacement; }

    static int run() {
        SharedGenericSet<Integer> holder = new SharedGenericSet<>();
        Map<String, Set<Integer>> table = new HashMap<>();
        holder.store(table);
        holder.add(1);
        table.get("x").add(2);
        Set<Integer> replacement = new LinkedHashSet<>();
        replacement.add(9);
        holder.replace(replacement);
        holder.add(10);
        return table.get("x").size() * 10 + replacement.size();
    }
}
