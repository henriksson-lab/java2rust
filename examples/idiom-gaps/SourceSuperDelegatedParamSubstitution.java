class SourceSuperDelegatedParamSubstitutionBase {
    enum Kind { FORMAT, INFO }

    Kind kind = null;
    int type = 0;

    SourceSuperDelegatedParamSubstitutionBase(Kind kind, int type) {
        this(kind, type, null);
    }

    SourceSuperDelegatedParamSubstitutionBase(Kind kind, int type, String ignored) {
        this.kind = kind;
        this.type = type;
    }
}

class SourceSuperDelegatedParamSubstitution extends SourceSuperDelegatedParamSubstitutionBase {
    SourceSuperDelegatedParamSubstitution() {
        super(Kind.FORMAT, 7);
    }
}
