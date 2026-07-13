class SuperConstructorState extends ParentWriter {
    private final State state;

    SuperConstructorState(Writer writer, String sampleName, State state) {
        super(writer, sampleName);
        this.state = Utils.nonNull(state);
    }
}

