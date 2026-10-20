import java.util.*;

class RetainedConstructionBase {
    static int order;
    static int failStep;
    final int first = mark(1);
    final List<Integer> values = new ArrayList<>();
    final int stamp = mark(2);
    RetainedConstructionBase(int value) {
        mark(3);
        values.add(value);
    }
    static int mark(int step) {
        order = order * 10 + step;
        if (failStep == step) throw new IllegalStateException("initialization");
        return step;
    }
    List<Integer> alias() { return values; }
}
class RetainedConstructionChild extends RetainedConstructionBase {
    final int childStamp = mark(4);
    RetainedConstructionChild(int value) {
        super(value);
        mark(5);
        values.add(9);
    }
}
class RetainedConstructorInitialization {
    static int run() {
        RetainedConstructionBase.order = 0;
        RetainedConstructionChild child = new RetainedConstructionChild(3);
        if (RetainedConstructionBase.order != 12345) return 1;
        List<Integer> alias = child.alias();
        alias.add(7);
        if (child.alias().size() != 3 || alias.get(0) != 3 || alias.get(1) != 9) return 2;
        RetainedConstructionBase.order = 0;
        RetainedConstructionBase.failStep = 2;
        try { new RetainedConstructionChild(4); return 3; }
        catch (IllegalStateException expected) { }
        return RetainedConstructionBase.order == 12 ? 0 : 4;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
