public class SuperMutatingCallReturning {
    public static String run() {
        Derived d = new Derived();
        int result = d.add(4);
        return result + "," + d.n;
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}

class BaseReturning {
    int n;

    int add(int value) {
        this.n += value;
        return this.n;
    }
}

class Derived extends BaseReturning {
    int add(int value) {
        return super.add(value) + this.n;
    }
}
