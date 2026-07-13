class ConstructorDelegationWithExtraWork {
    private final int value;

    ConstructorDelegationWithExtraWork() {
        this(1);
        audit();
    }

    ConstructorDelegationWithExtraWork(int value) {
        this.value = value;
    }

    void audit() {
    }
}

