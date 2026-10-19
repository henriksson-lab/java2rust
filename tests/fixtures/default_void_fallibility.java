interface FallibleStep {
    void execute() throws Exception;

    default void run() throws Exception {
        execute();
    }

    default void guarded(boolean enabled) throws Exception {
        if (!enabled) return;
        execute();
    }

    default void branches(boolean enabled) throws Exception {
        if (enabled) execute();
        else throw new IllegalArgumentException("disabled");
    }

    default void recovered() throws Exception {
        try {
            execute();
        } catch (Exception ignored) {
        }
    }
}
