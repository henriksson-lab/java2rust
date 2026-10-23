package samreturns;

interface Factory { Product create(int first, int second); }

class Product {
    final int value;
    Product(int value) { this.value = value; }
    synchronized int get() { return value; }
}

public class SharedSamReturnProbe {
    static Product invoke(Factory factory, int first, int second) {
        return factory.create(first, second);
    }
    static Factory factory(int offset) {
        return (first, second) -> first < second
            ? new Product(first + offset) : new Product(second - offset);
    }
    static int run() {
        Factory selected = factory(3);
        Product first = invoke(selected, 2, 8);
        Product second = invoke(selected, 9, 4);
        if (first.get() != 5 || second.get() != 1) return 1;
        Product fresh = invoke(selected, 2, 8);
        if (first == fresh || first == second) return 2;
        Product alias = first;
        if (alias != first || alias.get() != 5) return 3;
        Factory direct = (left, right) -> new Product(left + right);
        Factory block = (left, right) -> {
            if (left > right) return new Product(left);
            return new Product(right);
        };
        if (invoke(direct, 4, 7).get() != 11) return 4;
        if (invoke(block, 4, 7).get() != 7) return 5;
        if (invoke(block, 8, 2).get() != 8) return 6;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
