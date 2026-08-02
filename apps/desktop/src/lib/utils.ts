import { type ClassValue, clsx } from 'clsx'
import { twMerge } from 'tailwind-merge'

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

export type WithElementRef<T, Element extends HTMLElement = HTMLElement> = T & {
  ref?: Element | null
}

export type WithoutChild<T> = Omit<T, 'child'>
export type WithoutChildren<T> = Omit<T, 'children'>
export type WithoutChildrenOrChild<T> = Omit<T, 'children' | 'child'>
