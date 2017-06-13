//! Module containing the `Flattener` to process a program that it is R1CS-able.
//!
//! @file flatten.rs
//! @author Dennis Kuhnert <dennis.kuhnert@campus.tu-berlin.de>
//! @date 2017

use std::collections::{HashSet, HashMap};
use absy::*;
use absy::Expression::*;
use field::Field;

/// Flattener compute flattened program.
pub struct Flattener {
    /// Number of bits needed to represent the maximum value.
    bits: usize,
    /// Vector containing all used variables while processing the program.
    variables: HashSet<String>,
    /// Map of renamings for reassigned variables while processing the program.
    substitution: HashMap<String, String>,
    /// Index of the next introduced variable while processing the program.
    next_var_idx: usize,
}
impl Flattener {
    /// Returns a `Flattener` with fresh a fresh [substitution] and [variables].
    ///
    /// # Arguments
    ///
    /// * `bits` - Number of bits needed to represent the maximum value.
    pub fn new(bits: usize) -> Flattener {
        Flattener {
            bits: bits,
            variables: HashSet::new(),
            substitution: HashMap::new(),
            next_var_idx: 0
        }
    }

    /// Returns (condition true, condition false) `Identifier`s for the given condition.
    /// condition true = 1, if `condition` is true, 0 else
    /// condition false = 1, if `condition` is false, 0 else
    ///
    /// # Arguments
    ///
    /// * `statements_flattened` - Vector where new flattened statements can be added.
    /// * `condition` - `Condition` that will be flattened.
    fn flatten_condition<T: Field>(&mut self, functions_flattened: &Vec<Function<T>>, statements_flattened: &mut Vec<Statement<T>>, condition: Condition<T>) -> (Expression<T>, Expression<T>) {
        match condition {
            Condition::Lt(lhs, rhs) => {
                let lhs_flattened = self.flatten_expression(functions_flattened, statements_flattened, lhs);
                let rhs_flattened = self.flatten_expression(functions_flattened, statements_flattened, rhs);

                let lhs_name = format!("sym_{}", self.next_var_idx);
                self.next_var_idx += 1;
                statements_flattened.push(Statement::Definition(lhs_name.to_string(), lhs_flattened));
                let rhs_name = format!("sym_{}", self.next_var_idx);
                self.next_var_idx += 1;
                statements_flattened.push(Statement::Definition(rhs_name.to_string(), rhs_flattened));

                let subtraction_result = format!("sym_{}", self.next_var_idx);
                self.next_var_idx += 1;
                statements_flattened.push(Statement::Definition(
                    subtraction_result.to_string(),
                    Sub(
                        box Mult(box Number(T::from(2)), box Identifier(lhs_name.to_string())),
                        box Mult(box Number(T::from(2)), box Identifier(rhs_name.to_string()))
                    )
                ));
                for i in 0..self.bits-2 {
                    let new_name = format!("{}_b{}", &subtraction_result, i);
                    statements_flattened.push(Statement::Definition(
                        new_name.to_string(),
                        Mult(
                            box Identifier(new_name.to_string()),
                            box Identifier(new_name.to_string())
                        )
                    ));
                }
                let mut expr = Add(
                    box Identifier(format!("{}_b0", &subtraction_result)), // * 2^0
                    box Mult(
                        box Identifier(format!("{}_b1", &subtraction_result)),
                        box Number(T::from(2))
                    )
                );
                for i in 1..self.bits/2 {
                    expr = Add(
                        box expr,
                        box Add(
                            box Mult(
                                box Identifier(format!("{}_b{}", &subtraction_result, 2*i)),
                                box Number(T::from(2).pow(i))
                            ),
                            box Mult(
                                box Identifier(format!("{}_b{}", &subtraction_result, 2*i+1)),
                                box Number(T::from(2).pow(i))
                            ),
                        )
                    );
                }
                if self.bits % 2 == 1 {
                    expr = Add(
                        box expr,
                        box Mult(
                            box Identifier(format!("{}_b{}", &subtraction_result, self.bits - 3)),
                            box Number(T::from(2).pow(self.bits - 1))
                        )
                    )
                }
                statements_flattened.push(Statement::Definition(subtraction_result.to_string(), expr));

                let cond_true = format!("{}_b0", &subtraction_result);
                let cond_false = format!("sym_{}", self.next_var_idx);
                self.next_var_idx += 1;
                statements_flattened.push(Statement::Definition(cond_false.to_string(), Sub(box Number(T::one()), box Identifier(cond_true.to_string()))));
                (Identifier(cond_true), Identifier(cond_false))
            },
            Condition::Eq(lhs, rhs) => {
                // Wanted: (Y = (X != 0) ? 1 : 0)
                // X = a - b
                // # Y = if X == 0 then 0 else 1 fi
                // # M = if X == 0 then 1 else 1/X fi
                // Y == X * M
                // 0 == (1-Y) * X
                let name_x = format!("sym_{}", self.next_var_idx);
                self.next_var_idx += 1;
                let name_y = format!("sym_{}", self.next_var_idx);
                self.next_var_idx += 1;
                let name_m = format!("sym_{}", self.next_var_idx);
                self.next_var_idx += 1;
                let name_1_y = format!("sym_{}", self.next_var_idx);
                self.next_var_idx += 1;

                let x = self.flatten_expression(functions_flattened, statements_flattened, Sub(box lhs, box rhs));
                statements_flattened.push(Statement::Definition(name_x.to_string(), x));
                statements_flattened.push(Statement::Compiler(name_y.to_string(), IfElse(
                    box Condition::Eq(
                        Identifier(name_x.to_string()),
                        Number(T::zero())
                    ),
                    box Number(T::zero()),
                    box Number(T::one())
                )));
                statements_flattened.push(Statement::Compiler(name_m.to_string(), IfElse(
                    box Condition::Eq(
                        Identifier(name_x.to_string()),
                        Number(T::zero())
                    ),
                    box Number(T::one()),
                    box Div(box Number(T::one()), box Identifier(name_x.to_string()))
                )));
                statements_flattened.push(Statement::Condition(Identifier(name_y.to_string()), Mult(box Identifier(name_x.to_string()), box Identifier(name_m))));
                statements_flattened.push(Statement::Definition(name_1_y.to_string(), Sub(box Number(T::one()), box Identifier(name_y.to_string()))));
                statements_flattened.push(Statement::Condition(Number(T::zero()), Mult(box Identifier(name_1_y.to_string()), box Identifier(name_x))));

                (Identifier(name_1_y), Identifier(name_y))
            },
            _ => unimplemented!(),
        }
    }

    /// Returns a flattened `Expression` based on the given `expr`.
    ///
    /// # Arguments
    ///
    /// * `functions_flattened` - Vector containing already flattened functions.
    /// * `statements_flattened` - Vector where new flattened statements can be added.
    /// * `expr` - `Expresstion` that will be flattened.
    fn flatten_expression<T: Field>(&mut self, functions_flattened: &Vec<Function<T>>, statements_flattened: &mut Vec<Statement<T>>, expr: Expression<T>) -> Expression<T> {
        match expr {
            x @ Number(_) |
            x @ Identifier(_) => x,
            ref x @ Add(..) |
            ref x @ Sub(..) |
            ref x @ Mult(..) |
            ref x @ Div(..) if x.is_flattened() => x.clone(),
            Add(box left, box right) => {
                let left_flattened = self.flatten_expression(functions_flattened, statements_flattened, left);
                let right_flattened = self.flatten_expression(functions_flattened, statements_flattened, right);
                let new_left = if left_flattened.is_linear() {
                    left_flattened
                } else {
                    let new_name = format!("sym_{}", self.next_var_idx);
                    self.next_var_idx += 1;
                    statements_flattened.push(Statement::Definition(new_name.to_string(), left_flattened));
                    Identifier(new_name)
                };
                let new_right = if right_flattened.is_linear() {
                    right_flattened
                } else {
                    let new_name = format!("sym_{}", self.next_var_idx);
                    self.next_var_idx += 1;
                    statements_flattened.push(Statement::Definition(new_name.to_string(), right_flattened));
                    Identifier(new_name)
                };
                Add(box new_left, box new_right)
            },
            Sub(box left, box right) => {
                let left_flattened = self.flatten_expression(functions_flattened, statements_flattened, left);
                let right_flattened = self.flatten_expression(functions_flattened, statements_flattened, right);
                let new_left = if left_flattened.is_linear() {
                    left_flattened
                } else {
                    let new_name = format!("sym_{}", self.next_var_idx);
                    self.next_var_idx += 1;
                    statements_flattened.push(Statement::Definition(new_name.to_string(), left_flattened));
                    Identifier(new_name)
                };
                let new_right = if right_flattened.is_linear() {
                    right_flattened
                } else {
                    let new_name = format!("sym_{}", self.next_var_idx);
                    self.next_var_idx += 1;
                    statements_flattened.push(Statement::Definition(new_name.to_string(), right_flattened));
                    Identifier(new_name)
                };
                Sub(box new_left, box new_right)
            },
            Mult(box left, box right) => {
                let left_flattened = self.flatten_expression(functions_flattened, statements_flattened, left);
                let right_flattened = self.flatten_expression(functions_flattened, statements_flattened, right);
                let new_left = if left_flattened.is_linear() {
                    if let Sub(..) = left_flattened {
                        let new_name = format!("sym_{}", self.next_var_idx);
                        self.next_var_idx += 1;
                        statements_flattened.push(Statement::Definition(new_name.to_string(), left_flattened));
                        Identifier(new_name)
                    } else {
                        left_flattened
                    }
                } else {
                    let new_name = format!("sym_{}", self.next_var_idx);
                    self.next_var_idx += 1;
                    statements_flattened.push(Statement::Definition(new_name.to_string(), left_flattened));
                    Identifier(new_name)
                };
                let new_right = if right_flattened.is_linear() {
                    if let Sub(..) = right_flattened {
                        let new_name = format!("sym_{}", self.next_var_idx);
                        self.next_var_idx += 1;
                        statements_flattened.push(Statement::Definition(new_name.to_string(), right_flattened));
                        Identifier(new_name)
                    } else {
                        right_flattened
                    }
                } else {
                    let new_name = format!("sym_{}", self.next_var_idx);
                    self.next_var_idx += 1;
                    statements_flattened.push(Statement::Definition(new_name.to_string(), right_flattened));
                    Identifier(new_name)
                };
                Mult(box new_left, box new_right)
            },
            Div(box left, box right) => {
                let left_flattened = self.flatten_expression(functions_flattened, statements_flattened, left);
                let right_flattened = self.flatten_expression(functions_flattened, statements_flattened, right);
                let new_left = if left_flattened.is_linear() {
                    left_flattened
                } else {
                    let new_name = format!("sym_{}", self.next_var_idx);
                    self.next_var_idx += 1;
                    statements_flattened.push(Statement::Definition(new_name.to_string(), left_flattened));
                    Identifier(new_name)
                };
                let new_right = if right_flattened.is_linear() {
                    right_flattened
                } else {
                    let new_name = format!("sym_{}", self.next_var_idx);
                    self.next_var_idx += 1;
                    statements_flattened.push(Statement::Definition(new_name.to_string(), right_flattened));
                    Identifier(new_name)
                };
                Div(box new_left, box new_right)
            },
            Pow(base, exponent) => {
                // TODO currently assuming that base is number or variable
                match exponent {
                    box Number(ref x) if x > &T::one() => {
                        match base {
                            box Identifier(ref var) => {
                                let id = if x > &T::from(2) {
                                    let tmp_expression = self.flatten_expression(
                                        functions_flattened,
                                        statements_flattened,
                                        Pow(
                                            box Identifier(var.to_string()),
                                            box Number(x.clone() - T::one())
                                        )
                                    );
                                    let new_name = format!("sym_{}", self.next_var_idx);
                                    self.next_var_idx += 1;
                                    statements_flattened.push(Statement::Definition(new_name.to_string(), tmp_expression));
                                    new_name
                                } else {
                                    var.to_string()
                                };
                                Mult(
                                    box Identifier(id.to_string()),
                                    box Identifier(var.to_string())
                                )
                            },
                            box Number(var) => Mult(
                                box Number(var.clone()),
                                box Number(var)
                            ),
                            _ => panic!("Only variables and numbers allowed in pow base")
                        }
                    }
                    _ => panic!("Expected number > 1 as pow exponent"),
                }
            },
            IfElse(box condition, consequent, alternative) => {
                let (cond_true, cond_false) = self.flatten_condition(functions_flattened, statements_flattened, condition);
                // (condition_true * consequent) + (condition_false * alternatuve)
                self.flatten_expression(
                    functions_flattened,
                    statements_flattened,
                    Add(
                        box Mult(box cond_true, consequent),
                        box Mult(box cond_false, alternative)
                    )
                )
            },
            FunctionCall(ref id, ref params) => {
                for funct in functions_flattened {
                    if funct.id == *id && funct.arguments.len() == (*params).len() {
                        // add all flattened statements except return statement
                        for stat in funct.statements.clone() {
                            assert!(stat.is_flattened());
                            match stat {
                                Statement::Return(x) =>{
                                    // set return statements right side as expression result
                                    return x
                                },
                                _ => statements_flattened.push(stat),
                            }
                        }
                    } else {
                        panic!("Function definition for function {} with {} argument(s) not found.",funct.id , funct.arguments.len());
                    }
                }
                panic!("Should never happen.")
            },
        }
    }

    /// Returns a flattened `Function` based on the given `funct`.
    ///
    /// # Arguments
    ///
    /// * `functions_flattened` - Vector where new flattened statements can be added.
    /// * `funct` - `Function` that will be flattened.
    pub fn flatten_function<T: Field>(&mut self, functions_flattened: &mut Vec<Function<T>>, funct: Function<T>) -> Function<T> {
        let mut statements_flattened = Vec::new();
        for stat in funct.statements {
            match stat {
                Statement::Return(expr) => {
                    let expr_subbed = expr.apply_substitution(&self.substitution);
                    let rhs = self.flatten_expression(&functions_flattened, &mut statements_flattened, expr_subbed);
                    // TODO: Check if this is correct and needed
                    // use ~out variable for main, else use function name
                    //if funct.id == "main" {
                    //    self.variables.insert("~out".to_string());
                    //} else{
                    //    self.variables.insert(self.use_variable(funct.id));
                    //}
                    statements_flattened.push(Statement::Return(rhs));
                },
                Statement::Definition(id, expr) => {
                    let expr_subbed = expr.apply_substitution(&self.substitution);
                    let rhs = self.flatten_expression(&functions_flattened, &mut statements_flattened, expr_subbed);
                    statements_flattened.push(Statement::Definition(self.use_variable(id), rhs));
                },
                Statement::Condition(expr1, expr2) => {
                    let expr1_subbed = expr1.apply_substitution(&self.substitution);
                    let expr2_subbed = expr2.apply_substitution(&self.substitution);
                    let (lhs, rhs) = if expr1_subbed.is_linear() {
                        (expr1_subbed, self.flatten_expression(&functions_flattened, &mut statements_flattened, expr2_subbed))
                    } else if expr2_subbed.is_linear() {
                        (expr2_subbed, self.flatten_expression(&functions_flattened, &mut statements_flattened, expr1_subbed))
                    } else {
                        unimplemented!()
                    };
                    statements_flattened.push(Statement::Condition(lhs, rhs));
                },
                Statement::For(..) => unimplemented!(),
                s @ Statement::Compiler(..) => statements_flattened.push(s),
            }
        }
        Function { id: funct.id, arguments: funct.arguments, statements: statements_flattened }
    }

    /// Returns a flattened `Prog`ram based on the given `prog`.
    ///
    /// # Arguments
    ///
    /// * `prog` - `Prog`ram that will be flattened.
    pub fn flatten_program<T: Field>(&mut self, prog: Prog<T>) -> Prog<T> {
        let mut functions_flattened = Vec::new();
        self.variables = HashSet::new();
        self.substitution = HashMap::new();
        self.next_var_idx = 0;
        for func in prog.functions{
            let flattened_func = self.flatten_function(&mut functions_flattened, func);
            functions_flattened.push(flattened_func);
        }
        Prog { functions: functions_flattened}
    }


    /// Checks if the given name is a not used variable and returns a fresh variable.
    ///
    /// # Arguments
    ///
    /// * `name` - A String that holds the name of the variable
    fn use_variable(&mut self, name: String) -> String {
        let mut i = 0;
        let mut new_name = name.to_string();
        loop {
            if self.variables.contains(&new_name) {
                new_name = format!("{}_{}", &name, i);
                i += 1;
            } else {
                self.variables.insert(new_name.to_string());
                if i == 1 {
                    self.substitution.insert(name, new_name.to_string());
                } else if i > 1 {
                    self.substitution.insert(format!("{}_{}", name, i - 2), new_name.to_string());
                }
                return new_name;
            }
        }
    }
}
