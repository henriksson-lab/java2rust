interface SharedCounter {
    int next();
}

class CapturedCounter implements SharedCounter {
    class Token {}
    int value;

    CapturedCounter(int initial) {
        value = initial;
        new Token();
    }

    public int next() {
        return value++;
    }
}

class SharedConstructorAbi {
    static SharedCounter make(int initial) {
        return new CapturedCounter(initial);
    }

    static int run() {
        SharedCounter counter = make(7);
        return counter.next() * 10 + counter.next();
    }

    static int assigned() {
        SharedCounter counter = null;
        counter = new CapturedCounter(3);
        return counter.next();
    }
}
