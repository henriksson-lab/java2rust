/**
 * `new T[n]` for a reference element type must allocate `n` slots.
 *
 * It used to allocate an empty `Vec` — the right type, the wrong length — so the
 * first index panicked while the program compiled cleanly. Primitive arrays were
 * always fine, which is why nothing caught it.
 *
 * Java and the translation both print "4,3,1,ab". See
 * claude_audits/done/0034.
 */
public class ObjectArray {
    static class Node {
        int v;
    }

    public static String run() {
        Node[] cell = new Node[1];
        cell[0] = new Node();
        cell[0].v = 4;

        int[] nums = new int[2];
        nums[1] = 3;

        String[] words = new String[2];
        words[0] = "a";
        words[1] = "b";

        return cell[0].v + "," + nums[1] + "," + cell.length + "," + words[0] + words[1];
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
