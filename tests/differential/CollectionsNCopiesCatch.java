import java.util.Collections;

public class CollectionsNCopiesCatch {
    public static String run() {
        String ok = "" + Collections.nCopies(2, "x").size();
        try {
            Collections.nCopies(-1, "x");
            return ok + ":miss";
        } catch (IllegalArgumentException ex) {
            return ok + ":iae";
        }
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
