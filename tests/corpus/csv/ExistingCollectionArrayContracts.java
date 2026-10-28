import java.util.ArrayList;
import java.util.List;
import java.util.Arrays;

class ExistingArrayHolder {
    String[] held;
    ExistingArrayHolder(String[] value) { held = value; }
}

public class ExistingCollectionArrayContracts {
    static final String[] EMPTY = new String[0];
    static List<String> values = new ArrayList<>();
    static int events;
    static List<String> receiver(boolean present) {
        events = events * 10 + 1;
        return present ? values : null;
    }
    static String[] destination(boolean present, boolean add) {
        events = events * 10 + 2;
        if (add) values.add("late");
        return present ? EMPTY : null;
    }
    static String[] badDestination() {
        events = events * 10 + 2;
        throw new IllegalStateException("destination");
    }
    static int local() {
        List<String> source = new ArrayList<>();
        source.add("one"); source.add(null);
        String[] large = new String[] { "a", "b", "c", "tail" };
        String[] result = source.toArray(large);
        if (result != large || !large[0].equals("one") || large[1] != null || large[2] != null || !large[3].equals("tail")) return 1;
        result[0] = "changed";
        if (!large[0].equals("changed") || !source.get(0).equals("one")) return 2;
        String[] exact = new String[2];
        if (source.toArray(exact) != exact || exact[1] != null) return 3;
        String[] small = new String[] { "untouched" };
        String[] grown = source.toArray(small);
        if (grown == small || grown.length != 2 || !small[0].equals("untouched") || grown[1] != null) return 4;
        source.clear();
        if (source.toArray(EMPTY) != EMPTY) return 5;
        String[] emptyTarget = new String[] { "clear", "keep" };
        if (source.toArray(emptyTarget) != emptyTarget || emptyTarget[0] != null || !emptyTarget[1].equals("keep")) return 6;
        String[] backing = new String[] { "first", null, "last" };
        List<String> view = Arrays.asList(backing);
        if (view.toArray(backing) != backing || backing[1] != null || !backing[2].equals("last")) return 14;
        Object erased = grown;
        if (erased.getClass() != String[].class) return 15;
        source.add("kept");
        String[] heldTarget = new String[] { "old", "tail" };
        ExistingArrayHolder holder = new ExistingArrayHolder(source.toArray(heldTarget));
        if (holder.held != heldTarget || !holder.held[0].equals("kept") || holder.held[1] != null) return 16;
        holder.held[0] = "held";
        if (!heldTarget[0].equals("held")) return 17;
        return 0;
    }
    static int run() {
        int local = local(); if (local != 0) return local;
        events = 0;
        String[] grown = receiver(true).toArray(destination(true, true));
        if (events != 12 || grown == EMPTY || grown.length != 1 || !grown[0].equals("late")) return 7;
        events = 0;
        try { receiver(false).toArray(destination(true, false)); return 8; }
        catch (NullPointerException expected) { if (events != 12) return 9; }
        events = 0;
        try { receiver(true).toArray(destination(false, false)); return 10; }
        catch (NullPointerException expected) { if (events != 12) return 11; }
        events = 0;
        try { receiver(false).toArray(badDestination()); return 12; }
        catch (IllegalStateException expected) { if (events != 12) return 13; }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
