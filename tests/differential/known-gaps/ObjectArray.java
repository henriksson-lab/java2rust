/**
 * `new T[n]` for a reference element type allocates an empty Vec, so the first
 * index panics. Primitive arrays are fine. See claude_audits/0034.
 *
 * Java prints "4,3,1,ab". The translation panics at the first `cell[0]`.
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
