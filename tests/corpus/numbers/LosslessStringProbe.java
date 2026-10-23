/** Whole-path text migration: producers, views, substring and append must agree. */
public class LosslessStringProbe {
    static boolean same(String left, String right) { return left == right; }
    static String relay(String value) { return value; }
    public static int run() {
        String pair = "\ud83d\ude00";
        String high = relay(pair.substring(0,1));
        String low = pair.substring(1,2);
        if (high.length() != 1 || high.charAt(0) != '\ud83d' || low.charAt(0) != '\ude00') return 1;
        if (!same(pair.substring(0,pair.length()), pair)) return 2;
        String built = new StringBuilder().append(high).append(low).toString();
        if (!built.equals(pair)) return 3;
        if (!(high + low).equals(pair)) return 4;
        String literal = "\ud800";
        String fresh = new String(literal);
        if (!fresh.equals(literal) || same(fresh, literal) || fresh.charAt(0) != '\ud800') return 5;
        String fromArray = new String(new char[]{'A','\ud800','Z'});
        if (fromArray.length() != 3 || fromArray.charAt(1) != '\ud800') return 6;
        StringBuilder mutable = new StringBuilder(pair);
        String snapshot = mutable.toString();
        mutable.setCharAt(0,'x');
        if (!snapshot.equals(pair) || mutable.charAt(0) != 'x' || mutable.charAt(1) != '\ude00') return 7;
        if (!same("\ud83d" + "\ude00", pair)) return 8;
        if (!(high + '\ude00').equals(pair)) return 9;
        String absent = null;
        if (!(high + absent).equals(high + "null")) return 10;
        if (!("" + (1+2) + true + '\ud800').equals("3true\ud800")) return 11;
        if (same("" + high, high)) return 12;
        String rawHash = "\ud800A\udc00";
        if (rawHash.hashCode() != 53197791 || high.hashCode() != 0xd83d) return 13;
        if (!Boolean.parseBoolean("TrUe") || Boolean.parseBoolean(null)
            || Boolean.parseBoolean("true\ud800") || Boolean.parseBoolean(" true")) return 14;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
