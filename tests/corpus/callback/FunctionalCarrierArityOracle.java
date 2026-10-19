public class FunctionalCarrierArityOracle {
    static final class Left { final int value; Left(int value) { this.value = value; } }
    static final class Right { final int value; Right(int value) { this.value = value; } }
    static final class Answer { final int value; Answer(int value) { this.value = value; } }
    static final class Cell { int value; Cell(int value) { this.value = value; } }

    public static void main(String[] arguments) {
        FunctionSlot<Left, Right, Answer> once = new FunctionSlot<>(
            (left, right) -> new Answer(left.value + right.value));
        assert once.apply(new Left(20), new Right(22)).value == 42;
        assert FunctionalCarrierArity.sum(20, 22) == 42;
        Cell left = new Cell(1);
        Cell right = new Cell(2);
        FunctionSlot<Cell, Cell, Integer> repeated = new FunctionSlot<>((first, second) -> {
            first.value++;
            second.value++;
            return first.value + second.value;
        });
        assert repeated.applyTwice(left, right) == 7;
        assert left.value == 3 && right.value == 4;
        System.out.println("functional-carrier-arguments: ok");
    }
}
