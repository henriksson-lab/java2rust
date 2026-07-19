package idiom.gaps;

class MutableConfig {
    private int value;

    void setValue(int value) {
        this.value = value;
    }
}

class StaticConfigMutator {
    static void refresh(MutableConfig config) {
        ExternalApi.fill(config);
    }
}

public class FallbackStaticMutBorrowArgument {
    void run(MutableConfig config) {
        StaticConfigMutator.refresh(config);
    }
}
