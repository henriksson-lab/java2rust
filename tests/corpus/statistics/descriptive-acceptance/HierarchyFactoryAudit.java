interface HierarchyFactory<T, R> { R apply(T value); }
class HierarchyFactoryBase {
    int count;
    HierarchyFactoryBase(int value) { count = value; }
    int value() { return count; }
    void increment() { count++; }
}
class HierarchyFactoryMiddle extends HierarchyFactoryBase {
    HierarchyFactoryMiddle(int value) { super(value); }
    int value() { return count * 2; }
    static HierarchyFactoryMiddle middle(int[] values) { return HierarchyFactoryLeaf.of(values); }
}
final class HierarchyFactoryLeaf extends HierarchyFactoryMiddle {
    HierarchyFactoryLeaf(int value) { super(value); }
    int value() { return count * 3; }
    static HierarchyFactoryLeaf of(int[] values) { return new HierarchyFactoryLeaf(values[0]); }
}
public class HierarchyFactoryAudit {
    public static int run() {
        HierarchyFactory<int[], HierarchyFactoryBase> baseFactory = HierarchyFactoryLeaf::of;
        HierarchyFactory<int[], HierarchyFactoryMiddle> middleFactory = HierarchyFactoryLeaf::of;
        int[] input = {3};
        HierarchyFactoryBase first = baseFactory.apply(input);
        HierarchyFactoryMiddle second = middleFactory.apply(input);
        HierarchyFactoryBase alias = second;
        if (first.value() != 9 || second.value() != 9) return 1;
        first.increment();
        second.increment();
        if (first.value() != 12 || alias.value() != 12 || alias != second) return 2;
        if (HierarchyFactoryMiddle.middle(input).value() != 9) return 5;
        HierarchyFactoryBase direct = new HierarchyFactoryLeaf(5);
        if (direct.value() != 15 || first.value() != 12) return 3;
        HierarchyFactoryMiddle plain = new HierarchyFactoryMiddle(9);
        HierarchyFactoryBase plainAlias = plain;
        plain.increment();
        if (plainAlias.value() != 20) return 4;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
