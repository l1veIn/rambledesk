document.querySelector('[data-testid="review-signup"]').addEventListener('click', () => document.querySelector('#signup').showModal())
document.querySelector('[data-testid="review-plan"]').addEventListener('click', () => { document.querySelector('#signup').showModal(); history.pushState({}, '', '#team-plan') })
