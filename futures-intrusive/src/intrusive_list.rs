//! An intrusive list of data

use core::ops::{Deref, DerefMut};
use core::ptr::null_mut;
use core::marker::{PhantomPinned};

/// A node which carries data of type `T` and is stored in an intrusive list
#[derive(Debug)]
pub struct ListNode<T> {
    next: *mut ListNode<T>,
    /// The data which is associated to this list item
    data: T,
    /// Prevents `ListNode`s from being `Unpin`. They may never be moved, since
    /// the list semantics require addresses to be stable.
    _pin: PhantomPinned,
}

impl<T> ListNode<T> {
    /// Creates a new node with the associated data
    pub fn new(data: T) -> ListNode<T> {
        ListNode::<T> {
            next: null_mut(),
            data,
            _pin: PhantomPinned,
        }
    }
}

impl<T> Deref for ListNode<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.data
    }
}

impl<T> DerefMut for ListNode<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.data
    }
}

/// An intrusive linked list of nodes, where each node carries associated data
/// of type `T`.
#[derive(Debug)]
pub struct LinkedList<T> {
    head: *mut ListNode<T>,
}

impl<T> LinkedList<T> {
    /// Creates an empty linked list
    pub fn new() -> Self {
        LinkedList::<T> {
            head: null_mut(),
        }
    }

    /// Consumes the list and creates an iterator over the linked list.
    /// This function is only safe as long as all pointers which are stored inside
    /// the linked list are valid.
    pub unsafe fn into_iter(self) -> LinkedListIterator<T> {
        LinkedListIterator {
            current: self.head,
        }
    }

    /// Adds an item to the front of the linked list.
    /// The function is only safe as long as valid pointers are stored inside
    /// the linked list.
    pub unsafe fn add_front(&mut self, item: *mut ListNode<T>) {
        assert!(item != null_mut(), "Can not add null pointers");
        (*item).next = self.head;
        self.head = item;
    }

    /// Returns the last item in the linked list without removing it from the list
    /// The function is only safe as long as valid pointers are stored inside
    /// the linked list.
    /// The returned pointer is only guaranteed to be valid as long as the list
    /// is not mutated
    pub fn peek_last(&self) -> *mut ListNode<T> {
        let mut iter = self.head;
        if iter.is_null() {
            return null_mut();
        }

        unsafe {
            while !iter.is_null() {
                if (*iter).next.is_null() {
                    return iter;
                }
                iter = (*iter).next;
            }
        }

        panic!("No list item found");
    }

    /// Removes the last item from the linked list and returns it
    pub fn remove_last(&mut self) -> *mut ListNode<T> {
        let mut iter = self.head;
        if iter.is_null() {
            return null_mut();
        }

        let mut prev: *mut ListNode<T> = null_mut();
        unsafe {
            while !iter.is_null() {
                if (*iter).next.is_null() {
                    // This is the last iter in the list.
                    // Remove it from the list and return it.
                    if !prev.is_null() {
                        (*prev).next = null_mut();
                    }
                    else {
                        self.head = null_mut();
                    }

                    (*iter).next = null_mut();

                    return iter;
                }
                prev = iter;
                iter = (*iter).next;
            }
        }

        panic!("No list item found");
    }

    /// Removes all items from the linked list and returns a LinkedList which
    /// contains all the items.
    pub fn take(&mut self) -> LinkedList<T> {
        let head = self.head;
        self.head = null_mut();
        LinkedList::<T>{
            head,
        }
    }

    /// Returns whether the linked list doesn not contain any node
    pub fn is_empty(&self) -> bool {
        self.head.is_null()
    }

    /// Reverses the linked list.
    /// The function is only safe as long as valid pointers are stored inside
    /// the linked list.
    pub unsafe fn reverse(&mut self) {
        let mut rev_head: *mut ListNode<T> = null_mut();
        let mut iter = self.head;

        while !iter.is_null() {
            let curr = iter;
            iter = (*iter).next;

            (*curr).next = rev_head;
            rev_head = curr;
        }
        self.head = rev_head;
    }

    /// Removes the given item from the linked list.
    /// Returns whether the item was removed.
    /// The function is only safe as long as valid pointers are stored inside
    /// the linked list.
    pub unsafe fn remove(&mut self, item: *mut ListNode<T>) -> bool {
        if item.is_null() {
            return false;
        }

        if self.head == item {
            self.head = (*item).next;
            (*item).next = null_mut();
            return true;
        }

        let mut iter = self.head;
        while !iter.is_null() {
            if (*iter).next == item {
                (*iter).next = (*item).next;
                (*item).next = null_mut();
                return true;
            } else {
                iter = (*iter).next;
            }
        }

        false
    }
}

/// An iterator over an intrusively linked list
pub struct LinkedListIterator<T> {
    current: *mut ListNode<T>,
}

impl<T> Iterator for LinkedListIterator<T> {
    type Item = *mut ListNode<T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current.is_null() {
            return None;
        }

        let node = self.current;
        // Safety: This is safe as long as the linked list is intact, which was
        // already required through the unsafe when creating the iterator.
        unsafe {
            self.current = (*self.current).next;
        }
        Some(node)
    }
}

#[cfg(test)]
#[cfg(feature = "std")] // Tests make use of Vec at the moment
mod tests {
    use super::*;

    unsafe fn collect_list<T: Copy>(list: LinkedList<T>) -> Vec<T> {
        list.into_iter().map(|item|(*(*item).deref())).collect()
    }

    #[test]
    fn insert_and_iterate() {
        unsafe {
            let mut a = ListNode::new(5);
            let mut b = ListNode::new(7);
            let mut c = ListNode::new(31);

            let mut list = LinkedList::new();
            assert_eq!(true, list.is_empty());
            list.add_front(&mut c);
            assert_eq!(false, list.is_empty());
            list.add_front(&mut b);
            list.add_front(&mut a);

            let items: Vec<i32> = collect_list(list);
            assert_eq!([5,7,31].to_vec(), items);
        }
    }

    #[test]
    fn take_items() {
        unsafe {
            let mut a = ListNode::new(5);
            let mut b = ListNode::new(7);
            let mut c = ListNode::new(31);

            let mut list = LinkedList::new();
            list.add_front(&mut c);
            list.add_front(&mut b);
            list.add_front(&mut a);

            let taken = list.take();

            let items: Vec<i32> = collect_list(list);
            assert!(items.is_empty());
            let taken_items: Vec<i32> = collect_list(taken);
            assert_eq!([5,7,31].to_vec(), taken_items);
        }
    }

    #[test]
    fn peek_last() {
        unsafe {
            let mut a = ListNode::new(5);
            let mut b = ListNode::new(7);
            let mut c = ListNode::new(31);

            let mut list = LinkedList::new();
            list.add_front(&mut c);
            list.add_front(&mut b);
            list.add_front(&mut a);

            let last = list.peek_last();
            assert_eq!(31, *(*last).deref());
            list.remove_last();

            let last = list.peek_last();
            assert_eq!(7, *(*last).deref());
            list.remove_last();

            let last = list.peek_last();
            assert_eq!(5, *(*last).deref());
            list.remove_last();

            let last = list.peek_last();
            assert!(last.is_null());
        }
    }

    #[test]
    fn remove_last() {
        unsafe {
            let mut a = ListNode::new(5);
            let mut b = ListNode::new(7);
            let mut c = ListNode::new(31);

            let mut list = LinkedList::new();
            list.add_front(&mut c);
            list.add_front(&mut b);
            list.add_front(&mut a);

            list.remove_last();

            let items: Vec<i32> = collect_list(list);
            assert_eq!([5,7].to_vec(), items);
        }
    }

    #[test]
    fn remove_by_address() {
        unsafe {
            let mut a = ListNode::new(5);
            let mut b = ListNode::new(7);
            let mut c = ListNode::new(31);

            {
                // Remove first
                let mut list = LinkedList::new();
                list.add_front(&mut c);
                list.add_front(&mut b);
                list.add_front(&mut a);

                assert_eq!(true, list.remove(&mut a));
                // a should be no longer there and can't be removed twice
                assert_eq!(false, list.remove(&mut a));

                let items: Vec<i32> = collect_list(list);
                assert_eq!([7, 31].to_vec(), items);
            }

            {
                // Remove middle
                let mut list = LinkedList::new();
                list.add_front(&mut c);
                list.add_front(&mut b);
                list.add_front(&mut a);

                assert_eq!(true, list.remove(&mut b));

                let items: Vec<i32> = collect_list(list);
                assert_eq!([5, 31].to_vec(), items);
            }

            {
                // Remove last
                let mut list = LinkedList::new();
                list.add_front(&mut c);
                list.add_front(&mut b);
                list.add_front(&mut a);

                assert_eq!(true, list.remove(&mut c));

                let items: Vec<i32> = collect_list(list);
                assert_eq!([5, 7].to_vec(), items);
            }

            {
                // Remove missing
                let mut list = LinkedList::new();
                list.add_front(&mut b);
                list.add_front(&mut a);

                assert_eq!(false, list.remove(&mut c));
            }

            {
                // Remove null
                let mut list = LinkedList::new();
                list.add_front(&mut b);
                list.add_front(&mut a);

                assert_eq!(false, list.remove(null_mut()));
            }
        }
    }
}