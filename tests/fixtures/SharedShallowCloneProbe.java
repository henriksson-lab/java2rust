enum ShallowCloneTag { VALUE }
class ShallowCloneCell<E extends Enum<E>> implements Cloneable {
    E tag;
    int value;
    int[] shared = new int[] { 3 };
    ShallowCloneCell(E tag) { this.tag = tag; value = 4; }
    public ShallowCloneCell<E> clone() {
        try { return (ShallowCloneCell<E>) super.clone(); }
        catch (CloneNotSupportedException impossible) { throw new AssertionError(impossible); }
    }
}
class SharedShallowCloneProbe {
    static int run() {
        ShallowCloneCell<ShallowCloneTag> original = new ShallowCloneCell<>(ShallowCloneTag.VALUE);
        ShallowCloneCell<ShallowCloneTag> alias = original;
        alias.value = 7;
        ShallowCloneCell<ShallowCloneTag> copy = original.clone();
        if (copy == original || copy.value != 7 || copy.tag != ShallowCloneTag.VALUE) return 1;
        copy.value = 9;
        if (original.value != 7) return 2;
        copy.shared[0] = 11;
        if (original.shared[0] != 11 || copy.shared != original.shared) return 3;
        original.shared[0] = 13;
        if (copy.shared[0] != 13) return 4;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
