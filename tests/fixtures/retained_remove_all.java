import java.util.*;
class RetainedRemoveAll {
    static <T> void subtract(Set<T> target, List<T> source) {
        List<T> alias = source;
        alias.size();
        target.removeAll(source);
    }
    static int plainReceiver() {
        Set<String> target = new HashSet<>();
        target.add(null);
        target.add("a");
        target.add("b");
        List<String> source = new ArrayList<>();
        source.add("a");
        subtract(target, source);
        return target.size() * 10 + (target.contains(null) ? 1 : 0);
    }
    static int retainedReceiver() {
        List<String> target = new ArrayList<>();
        target.add("a"); target.add("b"); target.add("a");
        List<String> targetAlias = target;
        List<String> source = new ArrayList<>();
        List<String> sourceAlias = source;
        source.add(null); source.add("a");
        boolean changed = target.removeAll(source);
        return (changed ? 100 : 0) + targetAlias.size() * 10
            + (sourceAlias.size() == 2 ? 1 : 0);
    }
    static int plainList() {
        List<String> target = new ArrayList<>();
        target.add("a"); target.add("b");
        List<String> source = new ArrayList<>();
        List<String> alias = source;
        alias.add(null); alias.add("a");
        boolean changed = target.removeAll(source);
        return (changed ? 100 : 0) + target.size() * 10 + target.get(0).length();
    }
    static int selfRemoval() {
        List<String> values = new ArrayList<>();
        List<String> alias = values;
        values.add("x"); values.add("x");
        boolean first = values.removeAll(alias);
        boolean second = alias.removeAll(values);
        return (first ? 10 : 0) + (second ? 1 : 0) + values.size();
    }
}
