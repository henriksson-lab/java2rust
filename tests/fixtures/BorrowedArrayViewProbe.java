import java.util.Arrays;
import java.util.Collection;
import java.util.List;

class BorrowedArrayViewOwner {
    String[] backing = new String[] { "first", null };
    List<String> saved;
    int events;

    int later() {
        events = events * 10 + 2;
        backing[0] = "later";
        return 7;
    }

    int accept(Collection<String> values, int number) {
        events = events * 10 + 3;
        return values.size() * 10 + number;
    }

    int replace(List<String> values, int number) {
        events = events * 10 + 3;
        if (!values.get(0).equals("later") || values.get(1) != null) return -1;
        values.set(0, "written");
        saved = values;
        return number;
    }

    int fromVarargs(String... values) {
        events = events * 10 + 1;
        return accept(Arrays.asList(values), later());
    }

    int update() {
        events = 1;
        return replace(Arrays.asList(backing), later());
    }
}

class BorrowedArrayViewProbe {
    static int run() {
        BorrowedArrayViewOwner owner = new BorrowedArrayViewOwner();
        if (owner.fromVarargs(owner.backing) != 27 || owner.events != 123) return 1;
        if (owner.update() != 7 || owner.events != 123) return 2;
        if (!owner.backing[0].equals("written")) return 3;
        owner.backing[1] = "alias";
        if (!owner.saved.get(1).equals("alias")) return 4;
        owner.saved.set(1, null);
        if (owner.backing[1] != null) return 5;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
