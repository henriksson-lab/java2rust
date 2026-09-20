class CounterSink {
    int value;

    void add(int amount) {
        this.value = this.value + amount;
    }

    int read() {
        return this.value;
    }
}

interface Recorder {
    int step(int delta);
    int read();
}

public class AnonymousInterfaceCapturedCall {
    public static void main(String[] args) {
        CounterSink sink = new CounterSink();
        Recorder recorder = new Recorder() {
            int total = 1;

            public int step(int delta) {
                sink.add(this.total + delta);
                this.total = this.total + delta;
                return sink.read();
            }

            public int read() {
                return this.total;
            }
        };
        int first = recorder.step(2);
        int second = recorder.step(3);
        System.out.println(first * 100 + second * 10 + sink.read() + recorder.read());
    }
}
