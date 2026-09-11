public class ExplicitInheritanceUpcast {
    public static String run() {
        Account a = (Account) new Savings(100, 5);
        return a.label();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}

class Account {
    int amount;

    Account(int amount) {
        this.amount = amount;
    }

    String label() {
        return "account";
    }
}

class Savings extends Account {
    int rate;

    Savings(int amount, int rate) {
        super(amount);
        this.rate = rate;
    }

    String label() {
        return "savings";
    }
}
