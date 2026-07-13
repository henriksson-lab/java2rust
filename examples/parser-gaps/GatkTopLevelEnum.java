package parser.gaps;

public enum GatkTopLevelEnum {
    FIRST,
    SECOND;

    public boolean isFirst() {
        return this == FIRST;
    }
}
