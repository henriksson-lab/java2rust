public class PlatformCollectionHierarchyOracle {
    private static void check(Class<?> actual, Class<?> target, boolean expected) {
        boolean result = target.isAssignableFrom(actual);
        if (result != expected) {
            throw new AssertionError(actual.getName() + " -> " + target.getName() + ": " + result);
        }
        System.out.println(actual.getSimpleName() + "\t" + target.getSimpleName() + "\t" + result);
    }
    public static void main(String[] args) {
        check(java.util.AbstractList.class, Iterable.class, true);
        check(java.util.AbstractSequentialList.class, java.util.List.class, true);
        check(java.util.AbstractCollection.class, Iterable.class, true);
        check(java.util.AbstractCollection.class, java.util.List.class, false);
        check(java.util.LinkedHashSet.class, Iterable.class, true);
        check(java.nio.file.DirectoryStream.class, Iterable.class, true);
        check(java.util.Iterator.class, Iterable.class, false);
        check(java.util.Spliterator.class, Iterable.class, false);
        check(java.util.Enumeration.class, Iterable.class, false);
    }
}
