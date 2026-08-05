/**
 * A Java `static` field is **one variable shared by every thread**.
 *
 * Static storage used to lower to `thread_local!`, giving each thread its own
 * copy. Nothing failed: a shared counter, cache or registry silently became
 * per-thread, and a lazily initialised singleton was built once *per thread*.
 * The class-initialisation guard had the same defect, so a `static { … }` block
 * ran once per class **per thread** rather than once per class.
 *
 * This fixture is single-threaded on purpose — it pins the storage *identity*
 * and the once-per-class initialisation that the threaded case depends on,
 * without making the differential harness depend on scheduling.
 *
 * It deliberately avoids a nested class reading an outer `static` field. That
 * was a separate defect (0037, since fixed and pinned by `NestedStatic.java`),
 * and keeping the two apart is still right: this fixture is about storage
 * *identity*, and should not fail for a name-resolution reason.
 *
 * Java prints "7,abc,2,99,1".
 *
 * See claude_audits/done/0030-java-statics-are-thread-local.md.
 */
public class StaticSharing {
    static final int LIMIT = 7;
    static final String NAME = "abc";

    static int counter = 0;
    static int initRuns = 0;
    static int lazy = 0;

    static {
        initRuns = initRuns + 1;
        lazy = 99;
    }

    static void bump() {
        counter = counter + 1;
    }

    public static String run() {
        bump();
        bump();
        return LIMIT
                + "," + NAME
                + "," + counter
                + "," + lazy
                + "," + initRuns;
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
