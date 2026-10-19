import java.util.ArrayList;
import java.util.HashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

class RetainedNestedCollections {
    private Map<String, Set<Integer>> sets = new HashMap<>();

    static int sum(Iterable<Integer> values) {
        int result = 0;
        for (int value : values) { result += value; }
        return result;
    }

    static int mapAlias() {
        Set<Integer> values = new LinkedHashSet<>();
        values.add(1);
        Iterable<Integer> view = values;
        Map<String, Set<Integer>> table = new HashMap<>();
        table.put("x", values);
        values.add(2);
        table.get("x").add(3);
        return sum(view) * 10 + table.get("x").size();
    }

    static int listAlias() {
        List<Integer> row = new ArrayList<>();
        row.add(1);
        Iterable<Integer> view = row;
        List<List<Integer>> rows = new ArrayList<>();
        rows.add(row);
        row.add(2);
        rows.get(0).add(3);
        return sum(view) * 10 + rows.get(0).size();
    }

    static <T> Set<T> fresh() { return new LinkedHashSet<>(); }

    static int factoryAlias() {
        Set<Integer> values = RetainedNestedCollections.<Integer>fresh();
        Iterable<Integer> view = values;
        values.add(4);
        return sum(view);
    }

    static int fieldAlias() {
        RetainedNestedCollections store = new RetainedNestedCollections();
        Set<Integer> values = new LinkedHashSet<>();
        values.add(1);
        Iterable<Integer> view = values;
        store.sets.put("x", values);
        values.add(2);
        store.sets.get("x").add(3);
        return sum(view) * 10 + store.sets.get("x").size();
    }

    static int plainAlias() {
        List<Integer> row = new ArrayList<>();
        row.add(1);
        List<List<Integer>> rows = new ArrayList<>();
        rows.add(row);
        row.add(2);
        rows.get(0).add(3);
        return row.size() * 10 + rows.get(0).size();
    }

    static int extractedAlias() {
        List<Integer> row = new ArrayList<>();
        row.add(1);
        List<List<Integer>> rows = new ArrayList<>();
        rows.add(row);
        List<Integer> alias = rows.get(0);
        alias.add(2);
        return row.size() * 10 + alias.size();
    }

    static int crossOwnerAlias() {
        NestedCollectionHolder store = new NestedCollectionHolder();
        Set<Integer> values = new LinkedHashSet<>();
        values.add(1);
        store.values.put("x", values);
        values.add(2);
        store.values.get("x").add(3);
        return values.size() * 10 + store.values.get("x").size();
    }

    static int deepAlias() {
        List<Integer> row = new ArrayList<>();
        row.add(1);
        List<List<Integer>> rows = new ArrayList<>();
        rows.add(row);
        Map<String, List<List<Integer>>> table = new HashMap<>();
        table.put("x", rows);
        table.get("x").get(0).add(2);
        return row.size() * 10 + rows.get(0).size();
    }

    static int nullableRow() {
        List<List<Integer>> rows = new ArrayList<>();
        rows.add(null);
        List<Integer> row = new ArrayList<>();
        rows.add(row);
        rows.get(1).add(2);
        try {
            return rows.get(0).size();
        } catch (NullPointerException error) {
            return -row.size();
        }
    }

    static void storeRow(List<List<Integer>> rows, List<Integer> row) { rows.add(row); }
    static List<Integer> firstRow(List<List<Integer>> rows) { return rows.get(0); }

    static int callBoundary() {
        List<Integer> row = new ArrayList<>();
        row.add(1);
        List<List<Integer>> rows = new ArrayList<>();
        storeRow(rows, row);
        List<Integer> alias = firstRow(rows);
        alias.add(2);
        return row.size() * 10 + alias.size();
    }
}

class NestedCollectionHolder {
    Map<String, Set<Integer>> values = new HashMap<>();
}
