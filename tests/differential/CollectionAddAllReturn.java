import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

public class CollectionAddAllReturn {
    public static void main(String[] args) {
        System.out.print(run());
    }

    public static String run() {
        List<String> values = new ArrayList<String>();
        List<String> empty = new ArrayList<String>();
        boolean listChanged = values.addAll(Arrays.asList("a", "b"));
        boolean listUnchanged = values.addAll(empty);

        Set<String> names = new HashSet<String>();
        boolean setChanged = names.addAll(Arrays.asList("x", "x"));
        boolean setUnchanged = names.addAll(Arrays.asList("x"));

        return listChanged + "," + listUnchanged + "," + values.size()
                + "," + setChanged + "," + setUnchanged + "," + names.size();
    }
}
