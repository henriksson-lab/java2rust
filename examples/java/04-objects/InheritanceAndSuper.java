package examples.objects;

class BaseCounter {
    protected int count;

    public BaseCounter(int count) {
        this.count = count;
    }

    public int value() {
        return count;
    }
}

public class InheritanceAndSuper extends BaseCounter {
    public InheritanceAndSuper(int count) {
        super(count);
    }

    @Override
    public int value() {
        return super.value() + 1;
    }
}

