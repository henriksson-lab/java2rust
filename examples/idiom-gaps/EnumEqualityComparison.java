class EnumEqualityComparison {
    enum Kind {
        SNP,
        INDEL
    }

    boolean same(Kind left, Kind right) {
        return left == right;
    }

    boolean different(Kind left, Kind right) {
        return left != right;
    }
}
