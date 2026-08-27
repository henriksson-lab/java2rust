public class StringBuilderSubstringCatch {
    public static String run() {
        StringBuilder sb = new StringBuilder("abc");
        StringBuffer buf = new StringBuffer("wxyz");
        String ok = "";
        try {
            ok = sb.substring(1) + ":" + sb.substring(0, 2) + ":" + buf.substring(1, 3);
            sb.substring(4);
            return "miss";
        } catch (StringIndexOutOfBoundsException e) {
            return ok + ":caught";
        }
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
