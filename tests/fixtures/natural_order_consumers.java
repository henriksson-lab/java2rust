import java.util.ArrayList;
import java.util.Collections;
import java.util.Comparator;

class NaturalOrderConsumers {
    static Comparator<Integer> ascending() { return Comparator.naturalOrder(); }
    static Comparator<Integer> descending() { return Comparator.reverseOrder(); }
    static Comparator<Integer> utilityDescending() { return Collections.reverseOrder(); }
    static Comparator<String> words() { return Comparator.naturalOrder(); }
    static Comparator<String> reverseWords() { return Collections.reverseOrder(); }
    static int compared() {
        return ascending().compare(9, 2) * 100 + descending().compare(9, 2) * 10
            + utilityDescending().compare(9, 2);
    }
    static int wordCompared() { return words().compare("a", "d"); }
    static int reverseWordCompared() { return reverseWords().compare("a", "d"); }
    static int utf16Compared() { return words().compare("😀", ""); }
    static ArrayList<Integer> values() {
        ArrayList<Integer> values = new ArrayList<>();
        values.add(9); values.add(2); values.add(5);
        return values;
    }
    static int sorted() {
        ArrayList<Integer> values = values();
        values.sort(ascending());
        return values.get(0) * 100 + values.get(1) * 10 + values.get(2);
    }
    static int reverseSorted() {
        ArrayList<Integer> values = values();
        Collections.sort(values, Collections.reverseOrder());
        return values.get(0) * 100 + values.get(1) * 10 + values.get(2);
    }
    static int nullSorted() {
        ArrayList<Integer> values = values();
        values.sort(null);
        return values.get(0) * 100 + values.get(1) * 10 + values.get(2);
    }
    static int storedNullSorted() {
        Comparator<Integer> comparator = null;
        ArrayList<Integer> values = values();
        Collections.sort(values, comparator);
        return values.get(0) * 100 + values.get(1) * 10 + values.get(2);
    }
    static int nullableSorted(boolean reverse) {
        Comparator<Integer> comparator = null;
        if (reverse) comparator = descending();
        ArrayList<Integer> values = values();
        values.sort(comparator);
        return values.get(0) * 100 + values.get(1) * 10 + values.get(2);
    }
    static int extrema() {
        ArrayList<Integer> values = values();
        return Collections.min(values, null) * 100 + Collections.max(values, ascending());
    }
    static int emptyMinimum() {
        ArrayList<Integer> values = new ArrayList<>();
        try { return Collections.min(values, null); }
        catch (java.util.NoSuchElementException error) { return 7; }
    }
    static int emptySort() {
        ArrayList<Integer> values = new ArrayList<>();
        values.sort(null); Collections.sort(values, null);
        return values.size();
    }
    static int nullableCompare(boolean present) {
        Comparator<Integer> comparator = null;
        if (present) comparator = ascending();
        try { return comparator.compare(9, 2); }
        catch (NullPointerException error) { return 8; }
    }
    static int repeatedNullable(boolean reverse) {
        Comparator<Integer> comparator = null;
        if (reverse) comparator = descending();
        ArrayList<Integer> values = values();
        values.sort(comparator);
        int best = Collections.min(values, comparator);
        return values.get(0) * 100 + best;
    }
    static int sortNullableInput(ArrayList<Integer> values, Comparator<Integer> comparator) {
        values.sort(comparator);
        return values.get(0);
    }
    static int forwardedNull() {
        ArrayList<Integer> values = values();
        return sortNullableInput(values, null);
    }
}
