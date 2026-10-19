package sample.streams;

import java.util.*;

class CovariantStreamValues<T> {
    void copy(Set<? extends T> values, Set<T> selected) {
        Iterable<? extends T> retained = values;
        values.stream().filter(value -> value != null && selected.contains(value)).forEach(selected::add);
    }
    static int run() {
        Set<String> values = new HashSet<>();
        values.add("a"); values.add(null);
        Set<String> selected = new HashSet<>();
        selected.add("a");
        new CovariantStreamValues<String>().copy(values, selected);
        return selected.size();
    }
}
