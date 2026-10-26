import org.apache.commons.statistics.ranking.NaNStrategy;
import org.apache.commons.statistics.ranking.TiesStrategy;

public final class RankingEnums {
    static int calls;
    static NaNStrategy argument(boolean fail) {
        calls++;
        if (fail) throw new IllegalArgumentException("argument");
        return NaNStrategy.MINIMAL;
    }
    public static int metadata() {
        String[] nanNames = {"MINIMAL", "MAXIMAL", "REMOVED", "FIXED", "FAILED"};
        String[] tieNames = {"SEQUENTIAL", "MINIMUM", "MAXIMUM", "AVERAGE", "RANDOM"};
        NaNStrategy[] nan = NaNStrategy.values();
        TiesStrategy[] ties = TiesStrategy.values();
        if (nan.length != 5 || ties.length != 5) return 1;
        for (int i = 0; i < 5; i++) {
            if (nan[i].ordinal() != i || !nan[i].name().equals(nanNames[i])) return 2;
            if (!nan[i].toString().equals(nanNames[i])) return 3;
            if (ties[i].ordinal() != i || !ties[i].name().equals(tieNames[i])) return 4;
            if (!ties[i].toString().equals(tieNames[i])) return 5;
            if (NaNStrategy.valueOf(new String(nanNames[i])) != nan[i]) return 6;
            if (TiesStrategy.valueOf(new String(tieNames[i])) != ties[i]) return 7;
        }
        return 0;
    }
    public static int comparisons() {
        NaNStrategy[] nan = NaNStrategy.values();
        TiesStrategy[] ties = TiesStrategy.values();
        for (int i = 0; i < 5; i++) for (int j = 0; j < 5; j++) {
            if (nan[i].compareTo(nan[j]) != i - j) return 1;
            if (ties[i].compareTo(ties[j]) != i - j) return 2;
        }
        return 0;
    }
    public static int lookup() {
        String[] invalid = {"", "minimal", "NONE", "MINIMAL\u0000", "MINI\uD800"};
        for (String name : invalid) {
            try { NaNStrategy.valueOf(name); return 1; }
            catch (IllegalArgumentException expected) {
                if (!("No enum constant org.apache.commons.statistics.ranking.NaNStrategy." + name).equals(expected.getMessage())) return 2;
            }
            try { TiesStrategy.valueOf(name); return 3; }
            catch (IllegalArgumentException expected) {
                if (!("No enum constant org.apache.commons.statistics.ranking.TiesStrategy." + name).equals(expected.getMessage())) return 4;
            }
        }
        try { NaNStrategy.valueOf(null); return 5; }
        catch (NullPointerException expected) { if (!"Name is null".equals(expected.getMessage())) return 6; }
        try { TiesStrategy.valueOf(null); return 7; }
        catch (NullPointerException expected) { if (!"Name is null".equals(expected.getMessage())) return 8; }
        return 0;
    }
    public static int identity() {
        NaNStrategy value = NaNStrategy.MINIMAL;
        if (!value.equals(NaNStrategy.valueOf("MINIMAL")) || value.equals(null)) return 1;
        if (value.equals(NaNStrategy.MAXIMAL) || value.equals(TiesStrategy.SEQUENTIAL)) return 2;
        if (value.hashCode() != NaNStrategy.valueOf("MINIMAL").hashCode()) return 3;
        if (!value.getDeclaringClass().getName().equals("org.apache.commons.statistics.ranking.NaNStrategy")) return 4;
        if (!value.getClass().getName().equals("org.apache.commons.statistics.ranking.NaNStrategy")) return 5;
        if (value.getDeclaringClass() != NaNStrategy.class) return 6;
        return 0;
    }
    public static int arrays() {
        NaNStrategy[] first = NaNStrategy.values();
        NaNStrategy[] alias = first;
        NaNStrategy[] second = NaNStrategy.values();
        if (first == second || first != alias) return 1;
        first[0] = null;
        if (alias[0] != null || second[0] != NaNStrategy.MINIMAL) return 2;
        if (NaNStrategy.values()[0] != NaNStrategy.MINIMAL) return 3;
        return 0;
    }
    public static int nullOrder() {
        NaNStrategy absent = null;
        calls = 0;
        try { absent.compareTo(argument(false)); return 1; }
        catch (NullPointerException expected) { if (calls != 1) return 2; }
        try { absent.compareTo(argument(true)); return 3; }
        catch (IllegalArgumentException expected) { if (calls != 2 || !"argument".equals(expected.getMessage())) return 4; }
        try { NaNStrategy.MINIMAL.compareTo(null); return 5; }
        catch (NullPointerException expected) { }
        try { absent.name(); return 6; }
        catch (NullPointerException expected) { }
        try { absent.ordinal(); return 7; }
        catch (NullPointerException expected) { }
        return 0;
    }
    public static void main(String[] args) {
        System.out.println(metadata());
        System.out.println(comparisons());
        System.out.println(lookup());
        System.out.println(identity());
        System.out.println(arrays());
        System.out.println(nullOrder());
    }
}
