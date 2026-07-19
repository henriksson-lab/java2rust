interface Metric {
    int score(String sample);
    void merge(Metric other);
}

class CountMetric implements Metric {
    int value;

    public int score(String sample) {
        return this.value;
    }

    public void merge(Metric other) {
        this.value = this.value + other.score("x");
    }
}

class FixedMetric implements Metric {
    public int score(String sample) {
        return 1;
    }

    public void merge(Metric other) {
    }
}

class Registry {
    Metric current;

    public Metric fresh(CountMetric metric) {
        return metric;
    }

    public void install(CountMetric metric) {
        this.current = metric;
    }

    public int rank(Metric left, Metric right) {
        return left.score("a") + right.score("b");
    }
}
