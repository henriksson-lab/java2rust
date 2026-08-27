public class StringMethods {
    public static String run() {
        String s = "  Hello,World  ";
        String t = s.trim();
        String sub = t.substring(6);
        String sub2 = t.substring(0, 5);
        int idx = t.indexOf("World");
        int idx2 = t.indexOf('o');
        int last = t.lastIndexOf('o');
        String rep2 = t.replace("World", "There");
        String up = t.toUpperCase();
        String low = t.toLowerCase();
        boolean eq = t.equals("Hello,World");
        boolean sw = t.startsWith("Hell");
        boolean ew = t.endsWith("rld");
        int cmp = "abc".compareTo("abd");
        boolean ct = t.contains("lo,W");
        return t + "|" + sub + "|" + sub2 + "|" + idx + "|" + idx2 + "|" + last + "|" + rep2 + "|" + up + "|" + low + "|" + eq + "|" + sw + "|" + ew + "|" + cmp + "|" + ct + "|" + t.length() + "|" + t.isEmpty();
    }
    public static void main(String[] args) { System.out.println(run()); }
}
