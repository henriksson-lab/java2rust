public class ListProjectionBoundariesOracle {
    public static void main(String[] args) {
        if (ListProjectionBoundaries.genericElement() != 10) {
            throw new AssertionError("generic List type-test/get");
        }
        int aliases = ListProjectionBoundaries.aliases();
        int nullable = ListProjectionBoundaries.nullCast();
        int rejected = ListProjectionBoundaries.rejectedSet();
        int elements = ListProjectionBoundaries.nullableElement();
        if (aliases != 34 || nullable != 7 || rejected != 7 || elements != 110) {
            throw new AssertionError("List boundary semantics");
        }
        System.out.println(aliases + "\t" + nullable + "\t" + rejected + "\t" + elements);
    }
}
