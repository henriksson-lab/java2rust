public class PolymorphicListUpcast {
    public static String run() {
        java.util.ArrayList<Account> accounts = new java.util.ArrayList<Account>();
        accounts.add(new Savings(100, 5));
        Account a = accounts.get(0);
        return a.label() + "," + a.balance();
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

    int balance() {
        return amount;
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

    int balance() {
        return amount + rate;
    }
}
