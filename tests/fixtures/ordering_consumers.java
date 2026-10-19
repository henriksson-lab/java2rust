import java.util.ArrayList;
import java.util.Collections;
import java.util.Comparator;
import java.util.NoSuchElementException;

class OrderingConsumers {
    static boolean failed;
    static boolean afterFailure;
    static int key(int value) {
        if (failed) afterFailure = true;
        if (value < 0) {
            failed = true;
            throw new IllegalArgumentException("negative key");
        }
        return value / 10;
    }
    static Comparator<Integer> order() {
        return Comparator.comparingInt(OrderingConsumers::key);
    }
    static ArrayList<Integer> values() {
        ArrayList<Integer> values = new ArrayList<>();
        values.add(21);
        values.add(11);
        values.add(22);
        values.add(12);
        return values;
    }
    static int listSorted() {
        failed = false;
        ArrayList<Integer> values = values();
        values.sort(order());
        return values.get(0) * 1000000 + values.get(1) * 10000
            + values.get(2) * 100 + values.get(3);
    }
    static int collectionsSorted() {
        failed = false;
        ArrayList<Integer> values = values();
        Collections.sort(values, order());
        return values.get(0) * 1000000 + values.get(1) * 10000
            + values.get(2) * 100 + values.get(3);
    }
    static int extrema() {
        failed = false;
        ArrayList<Integer> values = values();
        return Collections.max(values, order()) * 100 + Collections.min(values, order());
    }
    static int caughtSort() {
        failed = false;
        afterFailure = false;
        ArrayList<Integer> values = values();
        values.add(-1);
        try {
            values.sort(order());
            return values.get(0);
        } catch (IllegalArgumentException error) {
            return afterFailure ? -2 : 9;
        }
    }
    static int caughtCollectionsSort() {
        failed = false;
        afterFailure = false;
        ArrayList<Integer> values = values();
        values.add(-1);
        try {
            Collections.sort(values, order());
            return values.get(0);
        } catch (IllegalArgumentException error) {
            return afterFailure ? -2 : 9;
        }
    }
    static int caughtMaximum() {
        failed = false;
        afterFailure = false;
        ArrayList<Integer> values = values();
        values.add(-1);
        values.add(99);
        try {
            return Collections.max(values, order());
        } catch (IllegalArgumentException error) {
            return afterFailure ? -2 : 9;
        }
    }
    static int emptyMinimum() {
        ArrayList<Integer> values = new ArrayList<>();
        try {
            return Collections.min(values, order());
        } catch (NoSuchElementException error) {
            return 7;
        }
    }
}
