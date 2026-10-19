import java.util.Arrays;
import java.util.ArrayList;
import java.util.ConcurrentModificationException;
import java.util.Iterator;
import java.util.LinkedHashSet;
import java.util.List;

public class RetainedCollectionIterableOracle {
    public static void main(String[] args) {
        int[] actual = {
            RetainedCollectionIterable.listAlias(),
            RetainedCollectionIterable.setAlias(),
            RetainedCollectionIterable.supplierAlias(),
            RetainedCollectionIterable.iteratorRemoval(),
            RetainedCollectionIterable.iteratorFailFast(),
            RetainedCollectionIterable.listDowncast(),
            RetainedCollectionIterable.nonListBranch()
        };
        int[] expected = {20, 7, 23, 14, -7, 4, 6};
        if (!Arrays.equals(actual, expected)) {
            throw new AssertionError(Arrays.toString(actual));
        }
        List<Integer> list = new ArrayList<>(Arrays.asList(3, 4));
        Iterator<Integer> first = list.iterator();
        Iterator<Integer> second = list.iterator();
        first.next();
        second.next();
        first.remove();
        // ArrayList.hasNext compares cursor and current size without checking
        // modCount; next still detects modification even if hasNext is false.
        if (second.hasNext()) { throw new AssertionError("list cursor/size"); }
        try {
            second.next();
            throw new AssertionError("list next must detect modification");
        } catch (ConcurrentModificationException expectedError) { }

        LinkedHashSet<Integer> set = new LinkedHashSet<>();
        Iterator<Integer> empty = set.iterator();
        set.add(1);
        // Hash/linked set iterators retain their already-discovered next node.
        if (empty.hasNext()) { throw new AssertionError("set next-node state"); }
        try {
            empty.next();
            throw new AssertionError("set next must detect modification");
        } catch (ConcurrentModificationException expectedError) { }
        try {
            empty.remove();
            throw new AssertionError("remove before next must fail");
        } catch (IllegalStateException expectedError) { }
        System.out.println(Arrays.toString(actual));
    }
}
