import inquirer from 'inquirer'

/**
 * Prompt for a string.
 * Ask's a question and expects user to answer with a string.
 *
 * @param {string} question The message to show before input.
 * @param {string} suggested Suggested answer.
 * @returns {string} The written answer.
 */
export const promptForString = async (
  question: string,
  suggested?: string
): Promise<string> => {
  const response = await inquirer
    .prompt({
      type: 'input',
      name: 'value',
      default: suggested,
      message: question,
    })
    .then()

  return response.value
}

/**
 * Prompt for confirmation.
 * Ask's a question and expects user to answer with a yes or no.
 *
 * @param {string} question The message to show before input.
 * @returns {string} The written answer.
 */
export const promptForConfirmation = async (
  question: string
): Promise<string> => {
  const response = await inquirer
    .prompt({
      type: 'confirm',
      name: 'value',
      message: question,
    })
    .then()

  return response.value
}

/**
 * Render a list of options to select.
 * Ask's a question and expects user to answer with one of the predefined options.
 *
 * @param {string} question The message to show before input.
 * @param {string[]} options An array with options.
 * @returns {string} The selected option.
 */
export const selectAnOption = async (
  question: string,
  options: Array<Array<string>>
): Promise<string> => {
  const choices: string[] = options.map((o) => o[1])

  const selectedText = await inquirer
    .prompt({
      type: 'list',
      name: 'value',
      message: question,
      choices: choices,
    })
    .then((response) => response.value)

  const selectedOption = options.find((option) => option[1] === selectedText)
  const selectedKey = selectedOption ? selectedOption[0] : undefined

  if (!selectedKey) {
    throw new Error('No option was selected')
  }

  return selectedKey
}
