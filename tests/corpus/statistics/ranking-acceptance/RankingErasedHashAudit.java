import org.apache.commons.statistics.ranking.NaNStrategy;

public class RankingErasedHashAudit {
    public static int run() {
        Object erased = NaNStrategy.MINIMAL;
        Object alias = erased;
        if (erased.hashCode() != NaNStrategy.MINIMAL.hashCode()) return 1;
        if (alias.hashCode() != erased.hashCode()) return 2;
        Object absent = null;
        try { absent.hashCode(); return 3; }
        catch (NullPointerException expected) { }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
