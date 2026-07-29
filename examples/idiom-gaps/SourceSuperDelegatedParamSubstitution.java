class SourceSuperDelegatedParamSubstitutionBase {
    enum Kind { FORMAT, INFO }

    Kind kind = null;

    SourceSuperDelegatedParamSubstitutionBase(Kind kind) {
        this(kind, null);
    }

    SourceSuperDelegatedParamSubstitutionBase(Kind kind, String ignored) {
        this.kind = kind;
    }
}

class SourceSuperDelegatedParamSubstitution extends SourceSuperDelegatedParamSubstitutionBase {
    SourceSuperDelegatedParamSubstitution() {
        super(Kind.FORMAT);
    }
}
